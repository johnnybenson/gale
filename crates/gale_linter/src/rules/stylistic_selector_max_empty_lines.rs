use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Limit the number of adjacent empty lines within selectors.
///
/// Equivalent to `@stylistic/selector-max-empty-lines`.
pub struct StylisticSelectorMaxEmptyLines;

impl Rule for StylisticSelectorMaxEmptyLines {
    fn name(&self) -> &'static str {
        "@stylistic/selector-max-empty-lines"
    }

    fn description(&self) -> &'static str {
        "Limit the number of adjacent empty lines within selectors"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, _nodes: &[CssNode], context: &RuleContext) -> Vec<Diagnostic> {
        let max = context
            .primary_option()
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let source = context.source;
        let bytes = source.as_bytes();
        let len = bytes.len();
        let mut diagnostics = Vec::new();
        let mut i = 0;
        let mut brace_depth = 0;
        let mut in_selector = true;

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
                brace_depth += 1;
                in_selector = false;
                i += 1;
                continue;
            }

            if bytes[i] == b'}' {
                if brace_depth > 0 {
                    brace_depth -= 1;
                }
                in_selector = true;
                i += 1;
                continue;
            }

            if bytes[i] == b';' {
                in_selector = true;
                i += 1;
                continue;
            }

            // Count consecutive empty lines within selectors
            if in_selector && bytes[i] == b'\n' {
                let line_start = i;
                i += 1;
                let mut consecutive_empty = 0;
                while i < len {
                    // Check if this line is empty (only whitespace until newline)
                    let mut j = i;
                    while j < len && (bytes[j] == b' ' || bytes[j] == b'\t' || bytes[j] == b'\r') {
                        j += 1;
                    }
                    if j < len && bytes[j] == b'\n' {
                        consecutive_empty += 1;
                        i = j + 1;
                    } else {
                        break;
                    }
                }

                if consecutive_empty > max {
                    diagnostics.push(
                        Diagnostic::new(
                            self.name(),
                            format!(
                                "Expected no more than {} empty line(s)",
                                max
                            ),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(line_start, i - line_start)),
                    );
                }
                continue;
            }

            if !in_selector
                && bytes[i] != b' '
                && bytes[i] != b'\t'
                && bytes[i] != b'\n'
                && bytes[i] != b'\r'
            {
                // Non-whitespace inside a block -- still not in selector
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

    fn check(source: &str, max: u64) -> Vec<Diagnostic> {
        let rule = StylisticSelectorMaxEmptyLines;
        let opts = serde_json::json!(max);
        let ctx = RuleContext {
            file_path: "test.css",
            source,
            syntax: Syntax::Css,
            options: Some(&opts),
        };
        rule.check_root(&[], &ctx)
    }

    #[test]
    fn allows_no_empty_lines_in_selector() {
        let d = check("a,\nb { color: red; }", 0);
        assert!(d.is_empty());
    }

    #[test]
    fn reports_empty_line_in_selector_when_max_zero() {
        let d = check("a,\n\nb { color: red; }", 0);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("no more than 0"));
    }

    #[test]
    fn allows_one_empty_line_when_max_one() {
        let d = check("a,\n\nb { color: red; }", 1);
        assert!(d.is_empty());
    }

    #[test]
    fn reports_two_empty_lines_when_max_one() {
        let d = check("a,\n\n\nb { color: red; }", 1);
        assert_eq!(d.len(), 1);
    }
}
