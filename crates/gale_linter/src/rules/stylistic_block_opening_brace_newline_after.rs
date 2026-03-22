use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require a newline after opening braces.
///
/// Equivalent to `@stylistic/block-opening-brace-newline-after`.
pub struct StylisticBlockOpeningBraceNewlineAfter;

impl Rule for StylisticBlockOpeningBraceNewlineAfter {
    fn name(&self) -> &'static str {
        "@stylistic/block-opening-brace-newline-after"
    }

    fn description(&self) -> &'static str {
        "Require a newline after opening braces"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, _nodes: &[CssNode], context: &RuleContext) -> Vec<Diagnostic> {
        let option = context.primary_option_str().unwrap_or("always");
        let source = context.source;
        let bytes = source.as_bytes();
        let len = bytes.len();
        let mut diagnostics = Vec::new();
        let mut i = 0;

        while i < len {
            // Skip strings
            if bytes[i] == b'"' || bytes[i] == b'\'' {
                let quote = bytes[i];
                i += 1;
                while i < len && bytes[i] != quote {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
                continue;
            }

            // Skip comments
            if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }

            if bytes[i] == b'{' {
                let brace_pos = i;

                // Find matching closing brace to determine if block is multi-line
                let mut depth = 1;
                let mut j = i + 1;
                let mut block_is_multi_line = false;
                while j < len && depth > 0 {
                    if bytes[j] == b'{' {
                        depth += 1;
                    } else if bytes[j] == b'}' {
                        depth -= 1;
                    } else if bytes[j] == b'\n' {
                        block_is_multi_line = true;
                    }
                    j += 1;
                }

                // Check what follows the opening brace
                let after = brace_pos + 1;
                let has_newline = after < len
                    && (bytes[after] == b'\n'
                        || (bytes[after] == b'\r'
                            && after + 1 < len
                            && bytes[after + 1] == b'\n'));

                let should_check = match option {
                    "always" => true,
                    "always-multi-line" => block_is_multi_line,
                    "never-multi-line" => block_is_multi_line,
                    _ => false,
                };

                if should_check {
                    let expect_newline = option != "never-multi-line";
                    if expect_newline && !has_newline {
                        diagnostics.push(
                            Diagnostic::new(
                                self.name(),
                                "Expected newline after \"{\"".to_string(),
                            )
                            .severity(self.default_severity())
                            .span(Span::new(brace_pos, 1)),
                        );
                    } else if !expect_newline && has_newline {
                        diagnostics.push(
                            Diagnostic::new(
                                self.name(),
                                "Unexpected newline after \"{\"".to_string(),
                            )
                            .severity(self.default_severity())
                            .span(Span::new(brace_pos, 1)),
                        );
                    }
                }
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

    fn ctx(source: &str) -> RuleContext {
        RuleContext {
            file_path: "test.css",
            source,
            syntax: Syntax::Css,
            options: None,
        }
    }

    fn ctx_with_option<'a>(source: &'a str, opt: &'a serde_json::Value) -> RuleContext<'a> {
        RuleContext {
            file_path: "test.css",
            source,
            syntax: Syntax::Css,
            options: Some(opt),
        }
    }

    #[test]
    fn allows_newline_after_brace() {
        let source = "a {\n  color: red;\n}";
        let d = StylisticBlockOpeningBraceNewlineAfter.check_root(&[], &ctx(source));
        assert!(d.is_empty());
    }

    #[test]
    fn reports_missing_newline_after_brace() {
        let opt = serde_json::Value::String("always".to_string());
        let source = "a { color: red;\n}";
        let d = StylisticBlockOpeningBraceNewlineAfter.check_root(&[], &ctx_with_option(source, &opt));
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("Expected newline"));
    }

    #[test]
    fn always_multi_line_allows_single_line_without_newline() {
        let opt = serde_json::Value::String("always-multi-line".to_string());
        let source = "a { color: red; }";
        let d = StylisticBlockOpeningBraceNewlineAfter.check_root(&[], &ctx_with_option(source, &opt));
        assert!(d.is_empty());
    }

    #[test]
    fn always_multi_line_reports_multi_line_without_newline_after_brace() {
        let opt = serde_json::Value::String("always-multi-line".to_string());
        let source = "a { color: red;\nbackground: blue; }";
        let d = StylisticBlockOpeningBraceNewlineAfter.check_root(&[], &ctx_with_option(source, &opt));
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn skips_comments() {
        let source = "/* { no newline } */ a {\n  color: red;\n}";
        let d = StylisticBlockOpeningBraceNewlineAfter.check_root(&[], &ctx(source));
        assert!(d.is_empty());
    }
}
