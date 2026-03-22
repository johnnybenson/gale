use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Specify indentation.
///
/// Equivalent to `@stylistic/indentation`.
pub struct StylisticIndentation;

impl Rule for StylisticIndentation {
    fn name(&self) -> &'static str {
        "@stylistic/indentation"
    }

    fn description(&self) -> &'static str {
        "Specify indentation"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, _nodes: &[CssNode], context: &RuleContext) -> Vec<Diagnostic> {
        let primary = context.primary_option();
        let use_tab: bool;
        let indent_size: usize;

        match primary {
            Some(serde_json::Value::String(s)) if s == "tab" => {
                use_tab = true;
                indent_size = 1;
            }
            Some(serde_json::Value::Number(n)) => {
                use_tab = false;
                indent_size = n.as_u64().unwrap_or(2) as usize;
            }
            _ => {
                use_tab = false;
                indent_size = 2;
            }
        }

        let source = context.source;
        let bytes = source.as_bytes();
        let len = bytes.len();
        let mut diagnostics = Vec::new();
        let mut i = 0;
        let mut brace_depth: i32 = 0;
        let mut paren_depth: i32 = 0;
        let mut line_start = 0;
        // Track the last meaningful (non-whitespace, non-comment) character
        // to detect multi-line value continuation lines.
        // A line is a "continuation" if the previous meaningful char was not
        // `;`, `{`, or `}` — meaning we're inside a multi-line value,
        // multi-line @import, etc. Stylelint skips indentation checks on those.
        let mut last_meaningful_char: u8 = b';'; // start as if after a statement

        while i < len {
            // Skip strings
            if bytes[i] == b'"' || bytes[i] == b'\'' {
                let quote = bytes[i];
                last_meaningful_char = quote;
                i += 1;
                while i < len && bytes[i] != quote {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    if i < len && bytes[i] == b'\n' {
                        line_start = i + 1;
                    }
                    i += 1;
                }
                if i < len {
                    last_meaningful_char = bytes[i]; // closing quote
                    i += 1;
                }
                continue;
            }

            // Skip block comments
            if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    if bytes[i] == b'\n' {
                        line_start = i + 1;
                    }
                    i += 1;
                }
                if i + 1 < len {
                    i += 2;
                }
                // Don't update last_meaningful_char — comments don't count
                continue;
            }

            // Skip SCSS line comments
            if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/' {
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                // Don't update last_meaningful_char — comments don't count
                continue;
            }

            // Skip SCSS interpolation #{...}
            if bytes[i] == b'#' && i + 1 < len && bytes[i + 1] == b'{' {
                i += 2;
                let mut interp_depth = 1;
                while i < len && interp_depth > 0 {
                    if bytes[i] == b'{' {
                        interp_depth += 1;
                    } else if bytes[i] == b'}' {
                        interp_depth -= 1;
                    }
                    if interp_depth > 0 {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1;
                }
                last_meaningful_char = b'}';
                continue;
            }

            if bytes[i] == b'{' {
                brace_depth += 1;
                last_meaningful_char = b'{';
                i += 1;
                continue;
            }

            if bytes[i] == b'}' {
                brace_depth -= 1;
                if brace_depth < 0 {
                    brace_depth = 0;
                }
                last_meaningful_char = b'}';
                i += 1;
                continue;
            }

            // Track parenthesis depth (SCSS maps, function arguments)
            if bytes[i] == b'(' {
                paren_depth += 1;
                last_meaningful_char = b'(';
                i += 1;
                continue;
            }
            if bytes[i] == b')' {
                paren_depth -= 1;
                if paren_depth < 0 {
                    paren_depth = 0;
                }
                last_meaningful_char = b')';
                i += 1;
                continue;
            }

            if bytes[i] == b';' {
                last_meaningful_char = b';';
                i += 1;
                continue;
            }

            if bytes[i] == b'\n' {
                line_start = i + 1;
                i += 1;

                // Determine if the next line is a continuation of a
                // multi-line value. Continuation lines are skipped, matching
                // Stylelint's behavior. A line is a continuation when:
                //  - we are inside parentheses (paren_depth > 0), OR
                //  - the last meaningful char before the newline was not a
                //    statement terminator (`;`, `{`, `}`)
                let is_continuation = paren_depth > 0
                    || (last_meaningful_char != b';'
                        && last_meaningful_char != b'{'
                        && last_meaningful_char != b'}');

                // Now check the indentation of the next line
                let expected_depth = brace_depth as usize;
                let mut actual_indent = 0;
                let mut j = line_start;
                let mut wrong_char = false;

                // Count leading whitespace
                while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                    if use_tab {
                        if bytes[j] == b'\t' {
                            actual_indent += 1;
                        } else {
                            wrong_char = true;
                        }
                    } else if bytes[j] == b' ' {
                        actual_indent += 1;
                    } else {
                        wrong_char = true;
                    }
                    j += 1;
                }

                // Skip empty lines
                if j >= len || bytes[j] == b'\n' || bytes[j] == b'\r' {
                    continue;
                }

                // Skip continuation lines (multi-line values, @import lists, etc.)
                if is_continuation {
                    continue;
                }

                // A closing brace should be at parent level
                let expected = if j < len && bytes[j] == b'}' {
                    if expected_depth > 0 {
                        (expected_depth - 1) * indent_size
                    } else {
                        0
                    }
                } else {
                    expected_depth * indent_size
                };

                if wrong_char {
                    let indent_type = if use_tab { "tabs" } else { "spaces" };
                    diagnostics.push(
                        Diagnostic::new(
                            self.name(),
                            format!("Expected {indent_type} for indentation"),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(line_start, j - line_start)),
                    );
                } else if actual_indent != expected {
                    let unit = if use_tab { "tab" } else { "space" };
                    let plural = if expected != 1 { "s" } else { "" };
                    diagnostics.push(
                        Diagnostic::new(
                            self.name(),
                            format!("Expected indentation of {expected} {unit}{plural}",),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(line_start, j - line_start)),
                    );
                }

                continue;
            }

            // Track any other non-whitespace character as meaningful
            if bytes[i] != b' ' && bytes[i] != b'\t' && bytes[i] != b'\r' {
                last_meaningful_char = bytes[i];
            }

            i += 1;
        }

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::Syntax;

    fn ctx_with_option<'a>(source: &'a str, opt: &'a serde_json::Value) -> RuleContext<'a> {
        RuleContext {
            file_path: "test.css",
            source,
            syntax: Syntax::Css,
            options: Some(opt),
        }
    }

    #[test]
    fn allows_correct_2_space_indent() {
        let opt = serde_json::json!(2);
        let source = "a {\n  color: red;\n}";
        let d = StylisticIndentation.check_root(&[], &ctx_with_option(source, &opt));
        assert!(
            d.is_empty(),
            "got: {:?}",
            d.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn reports_wrong_indent() {
        let opt = serde_json::json!(2);
        let source = "a {\ncolor: red;\n}";
        let d = StylisticIndentation.check_root(&[], &ctx_with_option(source, &opt));
        assert!(!d.is_empty());
        assert!(d[0].message.contains("Expected indentation"));
    }

    #[test]
    fn allows_tab_indent() {
        let opt = serde_json::json!("tab");
        let source = "a {\n\tcolor: red;\n}";
        let d = StylisticIndentation.check_root(&[], &ctx_with_option(source, &opt));
        assert!(
            d.is_empty(),
            "got: {:?}",
            d.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn reports_spaces_when_tab_expected() {
        let opt = serde_json::json!("tab");
        let source = "a {\n  color: red;\n}";
        let d = StylisticIndentation.check_root(&[], &ctx_with_option(source, &opt));
        assert!(!d.is_empty());
        assert!(d[0].message.contains("tabs"));
    }

    #[test]
    fn allows_4_space_indent() {
        let opt = serde_json::json!(4);
        let source = "a {\n    color: red;\n}";
        let d = StylisticIndentation.check_root(&[], &ctx_with_option(source, &opt));
        assert!(d.is_empty());
    }

    #[test]
    fn skips_multiline_value_continuation() {
        let opt = serde_json::json!(2);
        // Multi-line value: continuation lines after `src:` should not be checked
        let source = "@font-face {\n  src:\n    url('a.woff2') format('woff2'),\n    url('a.woff') format('woff');\n  font-weight: normal;\n}";
        let d = StylisticIndentation.check_root(&[], &ctx_with_option(source, &opt));
        assert!(
            d.is_empty(),
            "got: {:?}",
            d.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn skips_multiline_import() {
        let opt = serde_json::json!(2);
        // Multi-line @import: continuation lines should not be checked
        let source = "@import\n  'foo',\n  'bar';";
        let d = StylisticIndentation.check_root(&[], &ctx_with_option(source, &opt));
        assert!(
            d.is_empty(),
            "got: {:?}",
            d.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn skips_multiline_value_with_parens() {
        let opt = serde_json::json!(2);
        // Multi-line value with nested parens (calc, var)
        let source = "a {\n  margin: calc(\n    var(--x) * -1\n  );\n  color: red;\n}";
        let d = StylisticIndentation.check_root(&[], &ctx_with_option(source, &opt));
        assert!(
            d.is_empty(),
            "got: {:?}",
            d.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn skips_multiline_transition() {
        let opt = serde_json::json!(2);
        // Multi-line transition property value
        let source = "a {\n  transition:\n    background-color 0.2s linear,\n    opacity 0.2s linear;\n  color: red;\n}";
        let d = StylisticIndentation.check_root(&[], &ctx_with_option(source, &opt));
        assert!(
            d.is_empty(),
            "got: {:?}",
            d.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }
}
