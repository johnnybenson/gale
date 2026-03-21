use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require an empty line before rules (except the first-nested).
///
/// Equivalent to Stylelint's `rule-empty-line-before` rule with "always" option.
/// Detection-only (no autofix).
pub struct RuleEmptyLineBefore;

impl Rule for RuleEmptyLineBefore {
    fn name(&self) -> &'static str {
        "rule-empty-line-before"
    }

    fn description(&self) -> &'static str {
        "Require an empty line before rules"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, nodes: &[CssNode], ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        check_nodes(self, nodes, ctx, true, &mut diags);
        diags
    }
}

fn check_nodes(
    rule_impl: &RuleEmptyLineBefore,
    nodes: &[CssNode],
    ctx: &RuleContext,
    is_root: bool,
    diags: &mut Vec<Diagnostic>,
) {
    for (i, node) in nodes.iter().enumerate() {
        if let CssNode::Style(style) = node {
            // Skip the very first node at root level
            let is_first = i == 0;
            // Also skip if only preceded by comments
            let is_first_meaningful =
                is_first || nodes[..i].iter().all(|n| matches!(n, CssNode::Comment(_)));

            if !is_first_meaningful || !is_root {
                let offset = style.span.offset;
                if offset > 0 && offset <= ctx.source.len() {
                    let before = &ctx.source[..offset];
                    let trimmed = before.trim_end_matches([' ', '\t']);
                    if !trimmed.ends_with("\n\n") && !trimmed.ends_with("\r\n\r\n") {
                        // Only report if this isn't the first child in a block
                        if !is_first_meaningful {
                            diags.push(
                                Diagnostic::new(
                                    rule_impl.name(),
                                    format!(
                                        "Expected empty line before rule \"{}\"",
                                        style.selector
                                    ),
                                )
                                .severity(rule_impl.default_severity())
                                .span(Span::new(style.span.offset, style.span.length)),
                            );
                        }
                    }
                }
            }
        }

        // Recurse into at-rules
        if let CssNode::AtRule(at_rule) = node {
            check_nodes(rule_impl, &at_rule.children, ctx, false, diags);
        }
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
            syntax: Syntax::Css,
            options: None,
        }
    }

    #[test]
    fn reports_missing_empty_line_before_rule() {
        let src = "a { color: red; }\nb { color: blue; }";
        let b_offset = src.find("b {").unwrap();
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(4, 10),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(0, 17),
            }),
            CssNode::Style(StyleRule {
                selector: "b".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "blue".to_string(),
                    span: ParserSpan::new(b_offset + 4, 11),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(b_offset, 18),
            }),
        ];
        let d = RuleEmptyLineBefore.check_root(&nodes, &make_ctx(src));
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("b"));
    }

    #[test]
    fn allows_empty_line_before_rule() {
        let src = "a { color: red; }\n\nb { color: blue; }";
        let b_offset = src.find("b {").unwrap();
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(4, 10),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(0, 17),
            }),
            CssNode::Style(StyleRule {
                selector: "b".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "blue".to_string(),
                    span: ParserSpan::new(b_offset + 4, 11),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(b_offset, 18),
            }),
        ];
        let d = RuleEmptyLineBefore.check_root(&nodes, &make_ctx(src));
        assert!(d.is_empty());
    }
}
