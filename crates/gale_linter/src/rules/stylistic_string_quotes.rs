use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Specify single or double quotes around strings.
///
/// Primary: "single" | "double"
pub struct StylisticStringQuotes;

impl Rule for StylisticStringQuotes {
    fn name(&self) -> &'static str {
        "@stylistic/string-quotes"
    }

    fn description(&self) -> &'static str {
        "Specify single or double quotes around strings"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, _nodes: &[CssNode], ctx: &RuleContext) -> Vec<Diagnostic> {
        let option = ctx.primary_option_str().unwrap_or("double");
        let mut diagnostics = Vec::new();
        let bytes = ctx.source.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        let (bad_quote, good_name) = match option {
            "single" => (b'"', "single"),
            _ => (b'\'', "double"),
        };

        while i < len {
            // Skip comments
            if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }

            if bytes[i] == bad_quote {
                let start = i;
                i += 1;
                // Walk to closing quote
                while i < len {
                    if bytes[i] == b'\\' && i + 1 < len {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == bad_quote {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                let span_len = i - start;
                diagnostics.push(
                    Diagnostic::new(
                        self.name(),
                        format!("Expected {good_name} quotes"),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(start, span_len)),
                );
                continue;
            } else if bytes[i] == b'\'' || bytes[i] == b'"' {
                // Skip over good quotes
                let quote = bytes[i];
                i += 1;
                while i < len {
                    if bytes[i] == b'\\' && i + 1 < len {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == quote {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
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

    fn check(source: &str, option: &str) -> Vec<Diagnostic> {
        let rule = StylisticStringQuotes;
        let opts = serde_json::json!(option);
        let ctx = RuleContext {
            file_path: "test.css",
            source,
            syntax: Syntax::Css,
            options: Some(&opts),
        };
        rule.check_root(&[], &ctx)
    }

    #[test]
    fn double_accepts_double_quotes() {
        let d = check("a { content: \"hello\"; }", "double");
        assert!(d.is_empty());
    }

    #[test]
    fn double_rejects_single_quotes() {
        let d = check("a { content: 'hello'; }", "double");
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("double"));
    }

    #[test]
    fn single_accepts_single_quotes() {
        let d = check("a { content: 'hello'; }", "single");
        assert!(d.is_empty());
    }

    #[test]
    fn single_rejects_double_quotes() {
        let d = check("a { content: \"hello\"; }", "single");
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("single"));
    }

    #[test]
    fn skips_comments() {
        let d = check("/* 'inside comment' */ a { content: \"ok\"; }", "double");
        assert!(d.is_empty());
    }
}
