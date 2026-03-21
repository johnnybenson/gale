use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
            Severity::Hint => write!(f, "hint"),
        }
    }
}

// ---------------------------------------------------------------------------
// Span
// ---------------------------------------------------------------------------

/// A byte-offset span into the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    /// Byte offset from the start of the source.
    pub offset: usize,
    /// Length in bytes.
    pub length: usize,
}

impl Span {
    pub fn new(offset: usize, length: usize) -> Self {
        Self { offset, length }
    }

    /// Create a span from a start (inclusive) and end (exclusive) byte offset.
    pub fn from_range(start: usize, end: usize) -> Self {
        debug_assert!(end >= start, "Span end must be >= start");
        Self {
            offset: start,
            length: end - start,
        }
    }

    /// The exclusive end byte offset.
    pub fn end(&self) -> usize {
        self.offset + self.length
    }
}

// ---------------------------------------------------------------------------
// SourceLocation
// ---------------------------------------------------------------------------

/// A human-readable location in a source file (1-indexed line and column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceLocation {
    /// 1-indexed line number.
    pub line: usize,
    /// 1-indexed column number.
    pub column: usize,
    /// Byte offset from the start of the source.
    pub offset: usize,
}

impl SourceLocation {
    pub fn new(line: usize, column: usize, offset: usize) -> Self {
        Self {
            line,
            column,
            offset,
        }
    }

    /// Resolve a byte offset into a `SourceLocation` given the full source text.
    pub fn from_offset(source: &str, offset: usize) -> Self {
        let mut line = 1;
        let mut col = 1;
        for (i, ch) in source.char_indices() {
            if i == offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        Self {
            line,
            column: col,
            offset,
        }
    }
}

/// Pre-built index for O(log n) byte-offset to line/column lookups.
pub struct SourceLineIndex {
    /// `line_starts[i]` is the byte offset where line `i` (0-indexed) begins.
    /// Line 0 always starts at byte 0.
    line_starts: Vec<usize>,
}

impl SourceLineIndex {
    /// Build a line index from the given source text.
    pub fn build(source: &str) -> Self {
        let mut line_starts = vec![0usize];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self { line_starts }
    }

    /// Convert a byte offset to a 1-indexed (line, column) pair.
    pub fn offset_to_location(&self, offset: usize) -> (usize, usize) {
        // Binary search for the line containing `offset`.
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(exact) => exact,
            Err(insert) => insert - 1,
        };
        let line = line_idx + 1; // 1-indexed
        let col = offset - self.line_starts[line_idx] + 1; // 1-indexed
        (line, col)
    }
}

// ---------------------------------------------------------------------------
// Edit & Fix
// ---------------------------------------------------------------------------

/// A single text replacement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edit {
    /// The span of text to replace.
    pub span: Span,
    /// The replacement text.
    pub new_text: String,
}

impl Edit {
    pub fn new(span: Span, new_text: impl Into<String>) -> Self {
        Self {
            span,
            new_text: new_text.into(),
        }
    }
}

/// An auto-fix that can be applied to resolve a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fix {
    pub description: String,
    pub edits: Vec<Edit>,
}

impl Fix {
    pub fn new(description: impl Into<String>, edits: Vec<Edit>) -> Self {
        Self {
            description: description.into(),
            edits,
        }
    }
}

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

/// A single lint diagnostic emitted by a rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// The rule that produced this diagnostic (e.g. `"color-no-invalid-hex"`).
    pub rule_name: String,
    /// Human-readable message.
    pub message: String,
    /// Severity level.
    pub severity: Severity,
    /// Location in the source text.
    pub span: Span,
    /// Path to the file this diagnostic belongs to (set by the runner).
    pub file_path: String,
    /// Optional auto-fix.
    pub fix: Option<Fix>,
}

impl Diagnostic {
    /// Start building a diagnostic for the given rule.
    pub fn new(rule_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule_name: rule_name.into(),
            message: message.into(),
            severity: Severity::Warning,
            span: Span::new(0, 0),
            file_path: String::new(),
            fix: None,
        }
    }

    pub fn severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    pub fn file_path(mut self, path: impl Into<String>) -> Self {
        self.file_path = path.into();
        self
    }

    pub fn fix(mut self, fix: Fix) -> Self {
        self.fix = Some(fix);
        self
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {} ({})", self.severity, self.message, self.rule_name)
    }
}

// ---------------------------------------------------------------------------
// LintResult
// ---------------------------------------------------------------------------

/// The result of linting a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintResult {
    /// Path to the file that was linted.
    pub file_path: String,
    /// All diagnostics found in this file.
    pub diagnostics: Vec<Diagnostic>,
    /// The original source code of the file.
    pub source: String,
}

impl LintResult {
    pub fn new(
        file_path: impl Into<String>,
        source: impl Into<String>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            source: source.into(),
            diagnostics,
        }
    }

    /// Returns `true` if there are no diagnostics.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Number of diagnostics with the given severity.
    pub fn count_by_severity(&self, severity: Severity) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_from_range() {
        let span = Span::from_range(10, 20);
        assert_eq!(span.offset, 10);
        assert_eq!(span.length, 10);
        assert_eq!(span.end(), 20);
    }

    #[test]
    fn source_location_from_offset() {
        let src = "abc\ndef\nghi";
        let loc = SourceLocation::from_offset(src, 5); // 'e' in "def"
        assert_eq!(loc.line, 2);
        assert_eq!(loc.column, 2);
    }

    #[test]
    fn diagnostic_builder() {
        let diag = Diagnostic::new("color-no-invalid-hex", "Invalid hex color")
            .severity(Severity::Error)
            .span(Span::new(12, 4))
            .file_path("test.css");

        assert_eq!(diag.rule_name, "color-no-invalid-hex");
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.span.offset, 12);
        assert_eq!(diag.file_path, "test.css");
        assert!(diag.fix.is_none());
    }

    #[test]
    fn diagnostic_with_fix() {
        let fix = Fix::new(
            "Replace with valid hex",
            vec![Edit::new(Span::new(12, 4), "#fff")],
        );
        let diag = Diagnostic::new("color-no-invalid-hex", "Invalid hex color").fix(fix);

        assert!(diag.fix.is_some());
        assert_eq!(diag.fix.as_ref().unwrap().edits.len(), 1);
    }

    #[test]
    fn lint_result_helpers() {
        let result = LintResult::new("test.css", "body { color: red; }", vec![]);
        assert!(result.is_empty());
        assert_eq!(result.count_by_severity(Severity::Error), 0);
    }
}
