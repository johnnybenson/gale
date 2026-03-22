use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Limit the number of adjacent empty lines.
///
/// Equivalent to Stylelint's `@stylistic/max-empty-lines` rule.
pub struct StylisticMaxEmptyLines;

impl Rule for StylisticMaxEmptyLines {
    fn name(&self) -> &'static str {
        "@stylistic/max-empty-lines"
    }

    fn description(&self) -> &'static str {
        "Limit the number of adjacent empty lines"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, _nodes: &[CssNode], context: &RuleContext) -> Vec<Diagnostic> {
        let max = context
            .primary_option()
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;

        let source = context.source;
        let bytes = source.as_bytes();
        let len = bytes.len();
        let mut diagnostics = Vec::new();

        // We need to skip strings and comments while counting empty lines
        let mut i = 0;
        let mut consecutive_empty = 0;
        let mut line_start = 0;
        let mut line_is_empty = true;

        while i < len {
            // Skip strings
            if bytes[i] == b'"' || bytes[i] == b'\'' {
                let quote = bytes[i];
                line_is_empty = false;
                i += 1;
                while i < len && bytes[i] != quote {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    if i < len && bytes[i] == b'\n' {
                        // Newline inside string - reset tracking
                        consecutive_empty = 0;
                        line_is_empty = true;
                        line_start = i + 1;
                    }
                    i += 1;
                }
                i += 1;
                continue;
            }

            // Skip comments
            if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                line_is_empty = false;
                i += 2;
                while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    if bytes[i] == b'\n' {
                        consecutive_empty = 0;
                        line_is_empty = true;
                        line_start = i + 1;
                    }
                    i += 1;
                }
                i += 2;
                continue;
            }

            if bytes[i] == b'\n' {
                if line_is_empty {
                    consecutive_empty += 1;
                } else {
                    consecutive_empty = 0;
                }

                if consecutive_empty > max {
                    diagnostics.push(
                        Diagnostic::new(
                            self.name(),
                            format!("Expected no more than {} empty line(s)", max),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(line_start, i - line_start)),
                    );
                }

                line_start = i + 1;
                line_is_empty = true;
                i += 1;
                continue;
            }

            if bytes[i] == b'\r' {
                i += 1;
                continue;
            }

            if bytes[i] != b' ' && bytes[i] != b'\t' {
                line_is_empty = false;
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

    fn ctx_with_max<'a>(source: &'a str, max: &'a serde_json::Value) -> RuleContext<'a> {
        RuleContext {
            file_path: "test.css",
            source,
            syntax: Syntax::Css,
            options: Some(max),
        }
    }

    #[test]
    fn allows_single_empty_line() {
        let max = serde_json::json!(1);
        let source = "a { }\n\nb { }";
        let d = StylisticMaxEmptyLines.check_root(&[], &ctx_with_max(source, &max));
        assert!(d.is_empty());
    }

    #[test]
    fn reports_too_many_empty_lines() {
        let max = serde_json::json!(1);
        let source = "a { }\n\n\nb { }";
        let d = StylisticMaxEmptyLines.check_root(&[], &ctx_with_max(source, &max));
        assert!(!d.is_empty());
        assert!(d[0].message.contains("no more than 1"));
    }

    #[test]
    fn allows_two_empty_lines_when_max_is_two() {
        let max = serde_json::json!(2);
        let source = "a { }\n\n\nb { }";
        let d = StylisticMaxEmptyLines.check_root(&[], &ctx_with_max(source, &max));
        assert!(d.is_empty());
    }

    #[test]
    fn reports_three_empty_lines_when_max_is_two() {
        let max = serde_json::json!(2);
        let source = "a { }\n\n\n\nb { }";
        let d = StylisticMaxEmptyLines.check_root(&[], &ctx_with_max(source, &max));
        assert!(!d.is_empty());
    }
}
