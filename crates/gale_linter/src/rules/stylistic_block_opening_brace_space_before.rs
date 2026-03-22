use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require or disallow a space before the opening brace of blocks.
///
/// Equivalent to `@stylistic/block-opening-brace-space-before`.
pub struct StylisticBlockOpeningBraceSpaceBefore;

impl Rule for StylisticBlockOpeningBraceSpaceBefore {
    fn name(&self) -> &'static str {
        "@stylistic/block-opening-brace-space-before"
    }

    fn description(&self) -> &'static str {
        "Require or disallow a space before the opening brace of blocks"
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
                if i < len {
                    i += 1;
                }
                continue;
            }

            // Skip comments
            if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 < len {
                    i += 2;
                }
                continue;
            }

            if bytes[i] == b'{' {
                let brace_pos = i;

                // Determine if the block is single-line or multi-line
                let is_single_line = {
                    let mut j = brace_pos + 1;
                    let mut found_newline = false;
                    let mut brace_depth = 1;
                    while j < len && brace_depth > 0 {
                        if bytes[j] == b'{' {
                            brace_depth += 1;
                        } else if bytes[j] == b'}' {
                            brace_depth -= 1;
                        } else if bytes[j] == b'\n' {
                            found_newline = true;
                        }
                        j += 1;
                    }
                    !found_newline
                };

                // Check what's before the brace
                let has_space_before = brace_pos > 0
                    && (bytes[brace_pos - 1] == b' ' || bytes[brace_pos - 1] == b'\t');
                let has_newline_before = brace_pos > 0 && bytes[brace_pos - 1] == b'\n';

                let should_have_space = match option {
                    "always" => true,
                    "never" => false,
                    "always-single-line" => is_single_line,
                    "always-multi-line" => !is_single_line,
                    _ => {
                        i += 1;
                        continue;
                    }
                };

                if should_have_space {
                    if !has_space_before && !has_newline_before {
                        diagnostics.push(
                            Diagnostic::new(self.name(), "Expected a space before \"{\"")
                                .severity(self.default_severity())
                                .span(Span::new(brace_pos, 1)),
                        );
                    }
                } else {
                    if has_space_before {
                        diagnostics.push(
                            Diagnostic::new(self.name(), "Unexpected space before \"{\"")
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

    fn ctx_with_option<'a>(source: &'a str, opt: &'a serde_json::Value) -> RuleContext<'a> {
        RuleContext {
            file_path: "test.css",
            source,
            syntax: Syntax::Css,
            options: Some(opt),
        }
    }

    #[test]
    fn always_allows_space_before_brace() {
        let opt = serde_json::json!("always");
        let source = "a { color: red; }";
        let d =
            StylisticBlockOpeningBraceSpaceBefore.check_root(&[], &ctx_with_option(source, &opt));
        assert!(d.is_empty());
    }

    #[test]
    fn always_reports_missing_space() {
        let opt = serde_json::json!("always");
        let source = "a{ color: red; }";
        let d =
            StylisticBlockOpeningBraceSpaceBefore.check_root(&[], &ctx_with_option(source, &opt));
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("Expected a space"));
    }

    #[test]
    fn never_allows_no_space() {
        let opt = serde_json::json!("never");
        let source = "a{ color: red; }";
        let d =
            StylisticBlockOpeningBraceSpaceBefore.check_root(&[], &ctx_with_option(source, &opt));
        assert!(d.is_empty());
    }

    #[test]
    fn never_reports_space() {
        let opt = serde_json::json!("never");
        let source = "a { color: red; }";
        let d =
            StylisticBlockOpeningBraceSpaceBefore.check_root(&[], &ctx_with_option(source, &opt));
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("Unexpected space"));
    }

    #[test]
    fn always_single_line_allows_space_for_single_line() {
        let opt = serde_json::json!("always-single-line");
        let source = "a { color: red; }";
        let d =
            StylisticBlockOpeningBraceSpaceBefore.check_root(&[], &ctx_with_option(source, &opt));
        assert!(d.is_empty());
    }

    #[test]
    fn always_multi_line_allows_no_space_for_single_line() {
        let opt = serde_json::json!("always-multi-line");
        let source = "a{ color: red; }";
        let d =
            StylisticBlockOpeningBraceSpaceBefore.check_root(&[], &ctx_with_option(source, &opt));
        assert!(d.is_empty());
    }

    #[test]
    fn always_multi_line_reports_missing_space_for_multiline() {
        let opt = serde_json::json!("always-multi-line");
        let source = "a{\n  color: red;\n}";
        let d =
            StylisticBlockOpeningBraceSpaceBefore.check_root(&[], &ctx_with_option(source, &opt));
        assert_eq!(d.len(), 1);
    }
}
