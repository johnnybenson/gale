use gale_diagnostics::{LintResult, Severity, SourceLineIndex, SourceLocation};
use owo_colors::OwoColorize;
use serde::Serialize;

// ---------------------------------------------------------------------------
// Formatter trait
// ---------------------------------------------------------------------------

/// Trait for formatting lint results into a displayable string.
pub trait Formatter {
    fn format(&self, results: &[LintResult]) -> String;
}

// ---------------------------------------------------------------------------
// Helper: compute_location
// ---------------------------------------------------------------------------

/// Converts a byte offset into a 1-indexed (line, column) pair.
pub fn compute_location(source: &str, offset: usize) -> (usize, usize) {
    let loc = SourceLocation::from_offset(source, offset);
    (loc.line, loc.column)
}

// ---------------------------------------------------------------------------
// TextFormatter
// ---------------------------------------------------------------------------

/// Human-readable formatter similar to Stylelint's string formatter.
///
/// ```text
/// src/app.css
///   2:3  ⚠  Unexpected empty block  block-no-empty
///
/// ✖ 1 problem (0 errors, 1 warning)
/// ```
pub struct TextFormatter;

impl Formatter for TextFormatter {
    fn format(&self, results: &[LintResult]) -> String {
        let mut output = String::new();
        let mut total_errors: usize = 0;
        let mut total_warnings: usize = 0;

        for result in results {
            if result.diagnostics.is_empty() {
                continue;
            }

            let line_index = SourceLineIndex::build(&result.source);

            output.push_str(&result.file_path.underline().to_string());
            output.push('\n');

            for diag in &result.diagnostics {
                let (line, col) = line_index.offset_to_location(diag.span.offset);

                let location = format!("{line}:{col}");
                let (icon, colored_message) = match diag.severity {
                    Severity::Error => {
                        total_errors += 1;
                        ("\u{2716}".red().to_string(), diag.message.red().to_string())
                    }
                    Severity::Warning => {
                        total_warnings += 1;
                        (
                            "\u{26A0}".yellow().to_string(),
                            diag.message.yellow().to_string(),
                        )
                    }
                    Severity::Info | Severity::Hint => {
                        total_warnings += 1;
                        (
                            "\u{26A0}".yellow().to_string(),
                            diag.message.yellow().to_string(),
                        )
                    }
                };

                let rule = diag.rule_name.dimmed();
                output.push_str(&format!(
                    "  {location:<8} {icon}  {colored_message}  {rule}\n"
                ));
            }

            output.push('\n');
        }

        let total = total_errors + total_warnings;
        if total > 0 {
            let problem_word = if total == 1 { "problem" } else { "problems" };
            let error_word = if total_errors == 1 { "error" } else { "errors" };
            let warning_word = if total_warnings == 1 {
                "warning"
            } else {
                "warnings"
            };

            let summary = format!(
                "\u{2716} {total} {problem_word} ({total_errors} {error_word}, {total_warnings} {warning_word})"
            );
            output.push_str(&summary.bold().to_string());
            output.push('\n');
        }

        output
    }
}

// ---------------------------------------------------------------------------
// JsonFormatter
// ---------------------------------------------------------------------------

/// JSON formatter matching Stylelint's JSON output format.
pub struct JsonFormatter;

#[derive(Serialize)]
struct JsonResult {
    source: String,
    warnings: Vec<JsonWarning>,
}

#[derive(Serialize)]
struct JsonWarning {
    line: usize,
    column: usize,
    rule: String,
    severity: String,
    text: String,
}

impl Formatter for JsonFormatter {
    fn format(&self, results: &[LintResult]) -> String {
        let json_results: Vec<JsonResult> = results
            .iter()
            .map(|result| {
                let line_index = SourceLineIndex::build(&result.source);
                let warnings = result
                    .diagnostics
                    .iter()
                    .map(|diag| {
                        let (line, column) = line_index.offset_to_location(diag.span.offset);
                        // Stylelint appends " (rule-name)" to every message text.
                        // We must replicate this for byte-for-byte identical JSON output.
                        let text = format!("{} ({})", diag.message, diag.rule_name);
                        JsonWarning {
                            line,
                            column,
                            rule: diag.rule_name.clone(),
                            severity: diag.severity.to_string(),
                            text,
                        }
                    })
                    .collect();

                JsonResult {
                    source: result.file_path.clone(),
                    warnings,
                }
            })
            .collect();

        serde_json::to_string(&json_results).unwrap_or_else(|_| "[]".to_string())
    }
}

// ---------------------------------------------------------------------------
// CompactFormatter
// ---------------------------------------------------------------------------

/// One-line-per-warning compact format.
///
/// ```text
/// src/app.css: line 2, col 3, warning - Unexpected empty block (block-no-empty)
/// ```
pub struct CompactFormatter;

impl Formatter for CompactFormatter {
    fn format(&self, results: &[LintResult]) -> String {
        let mut output = String::new();

        for result in results {
            let line_index = SourceLineIndex::build(&result.source);
            for diag in &result.diagnostics {
                let (line, col) = line_index.offset_to_location(diag.span.offset);
                output.push_str(&format!(
                    "{}: line {}, col {}, {} - {} ({})\n",
                    result.file_path, line, col, diag.severity, diag.message, diag.rule_name,
                ));
            }
        }

        output
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Create a formatter by name. Supported values: `"text"`, `"json"`, `"compact"`.
///
/// Defaults to `TextFormatter` for unknown format types.
pub fn create_formatter(format_type: &str) -> Box<dyn Formatter> {
    match format_type {
        "json" => Box::new(JsonFormatter),
        "compact" => Box::new(CompactFormatter),
        _ => Box::new(TextFormatter),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use gale_diagnostics::{Diagnostic, Span};

    fn sample_results() -> Vec<LintResult> {
        let source = "a {\n  \n}\n";
        let diag = Diagnostic::new("block-no-empty", "Unexpected empty block")
            .severity(Severity::Warning)
            .span(Span::new(4, 3))
            .file_path("src/app.css");

        vec![LintResult::new("src/app.css", source, vec![diag])]
    }

    #[test]
    fn text_formatter_output() {
        let formatter = TextFormatter;
        let output = formatter.format(&sample_results());
        assert!(output.contains("src/app.css"));
        assert!(output.contains("Unexpected empty block"));
        assert!(output.contains("block-no-empty"));
        assert!(output.contains("1 problem"));
    }

    #[test]
    fn json_formatter_output() {
        let formatter = JsonFormatter;
        let output = formatter.format(&sample_results());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["source"], "src/app.css");
        assert_eq!(arr[0]["warnings"][0]["rule"], "block-no-empty");
        assert_eq!(arr[0]["warnings"][0]["severity"], "warning");
    }

    #[test]
    fn compact_formatter_output() {
        let formatter = CompactFormatter;
        let output = formatter.format(&sample_results());
        assert!(output.contains(
            "src/app.css: line 2, col 1, warning - Unexpected empty block (block-no-empty)"
        ));
    }

    #[test]
    fn compute_location_basic() {
        let source = "abc\ndef\nghi";
        let (line, col) = compute_location(source, 5);
        assert_eq!(line, 2);
        assert_eq!(col, 2);
    }

    #[test]
    fn compute_location_start_of_file() {
        let (line, col) = compute_location("hello", 0);
        assert_eq!(line, 1);
        assert_eq!(col, 1);
    }

    #[test]
    fn create_formatter_returns_correct_types() {
        let _ = create_formatter("text");
        let _ = create_formatter("json");
        let _ = create_formatter("compact");
        let _ = create_formatter("unknown");
    }

    #[test]
    fn empty_results_produce_no_output() {
        let formatter = TextFormatter;
        let output = formatter.format(&[]);
        assert!(output.is_empty());
    }
}
