use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Warn when there's an empty line before a declaration inside a rule block.
///
/// Equivalent to Stylelint's `declaration-empty-line-before` rule with "never" option.
/// Detection-only (no autofix).
pub struct DeclarationEmptyLineBefore;

impl Rule for DeclarationEmptyLineBefore {
    fn name(&self) -> &'static str {
        "declaration-empty-line-before"
    }

    fn description(&self) -> &'static str {
        "Disallow empty lines before declarations"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let mut diags = Vec::new();

        for decl in &rule.declarations {
            let decl_start = decl.span.offset;
            if decl_start == 0 || decl_start > ctx.source.len() {
                continue;
            }

            // Look at the text before this declaration within the source.
            // Check if there's a blank line (two consecutive newlines) immediately before.
            let before = &ctx.source[..decl_start];
            // Trim trailing whitespace on each line to find \n\n pattern.
            let trimmed_end = before.trim_end_matches([' ', '\t']);
            if trimmed_end.ends_with("\n\n") || trimmed_end.ends_with("\r\n\r\n") {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Unexpected empty line before declaration \"{}\"",
                            decl.property
                        ),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(decl.span.offset, decl.span.length)),
                );
            }
        }
        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn make_ctx(source: &str) -> RuleContext {
        RuleContext {
            file_path: "t.css",
            source,
            syntax: Syntax::Css, options: None }
    }

    #[test]
    fn reports_empty_line_before_declaration() {
        let src = "a {\n  color: red;\n\n  display: block;\n}";
        // "display: block;" starts after the double newline.
        let display_offset = src.find("display").unwrap();
        let display_len = "display: block;".len();
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(src.find("color").unwrap(), "color: red;".len()),
                    important: false,
                },
                Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(display_offset, display_len),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, src.len()),
        });
        let d = DeclarationEmptyLineBefore.check(&node, &make_ctx(src));
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("display"));
    }

    #[test]
    fn allows_no_empty_line() {
        let src = "a {\n  color: red;\n  display: block;\n}";
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(src.find("color").unwrap(), "color: red;".len()),
                    important: false,
                },
                Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(
                        src.find("display").unwrap(),
                        "display: block;".len(),
                    ),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, src.len()),
        });
        let d = DeclarationEmptyLineBefore.check(&node, &make_ctx(src));
        assert!(d.is_empty());
    }
}
