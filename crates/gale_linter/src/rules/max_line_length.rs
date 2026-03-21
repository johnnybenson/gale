use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Warn when a line exceeds 120 characters.
///
/// Equivalent to Stylelint's `max-line-length` rule with default 120.
pub struct MaxLineLength;

const MAX_LENGTH: usize = 120;

impl Rule for MaxLineLength {
    fn name(&self) -> &'static str {
        "max-line-length"
    }

    fn description(&self) -> &'static str {
        "Limit the length of a line"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, _nodes: &[CssNode], context: &RuleContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut offset = 0;

        for (line_num, line) in context.source.split('\n').enumerate() {
            // Use the visible length (strip trailing \r if present)
            let visible = line.strip_suffix('\r').unwrap_or(line);
            let char_count = visible.chars().count();
            if char_count > MAX_LENGTH {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected line {num} to be no more than {MAX_LENGTH} characters, but found {char_count}",
                            num = line_num + 1,
                        ),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(offset, visible.len())),
                );
            }
            // +1 for the newline character
            offset += line.len() + 1;
        }

        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::Syntax;

    fn ctx_with_source(source: &str) -> RuleContext<'_> {
        RuleContext {
            file_path: "t.css",
            source,
            syntax: Syntax::Css,
        }
    }

    #[test]
    fn reports_long_line() {
        let long_line = "a".repeat(121);
        let source = format!(".foo {{ color: {}; }}", long_line);
        let d = MaxLineLength.check_root(&[], &ctx_with_source(&source));
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("121") || d[0].message.contains("120"));
    }

    #[test]
    fn allows_short_line() {
        let source = ".foo { color: red; }";
        let d = MaxLineLength.check_root(&[], &ctx_with_source(source));
        assert!(d.is_empty());
    }

    #[test]
    fn reports_correct_line_number() {
        let source = "a { }\n".to_string() + &"b".repeat(121);
        let d = MaxLineLength.check_root(&[], &ctx_with_source(&source));
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("line 2"));
    }
}
