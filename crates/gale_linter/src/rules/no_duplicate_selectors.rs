use std::collections::HashMap;

use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow duplicate selectors within a stylesheet.
///
/// Equivalent to Stylelint's `no-duplicate-selectors` rule.
pub struct NoDuplicateSelectors;

impl Rule for NoDuplicateSelectors {
    fn name(&self) -> &'static str {
        "no-duplicate-selectors"
    }

    fn description(&self) -> &'static str {
        "Disallow duplicate selectors within a stylesheet"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, nodes: &[CssNode], _context: &RuleContext) -> Vec<Diagnostic> {
        let mut seen: HashMap<String, ()> = HashMap::new();
        let mut diagnostics = Vec::new();

        collect_selectors(nodes, &mut seen, &mut diagnostics, self);

        diagnostics
    }
}

fn collect_selectors(
    nodes: &[CssNode],
    seen: &mut HashMap<String, ()>,
    diagnostics: &mut Vec<Diagnostic>,
    rule: &NoDuplicateSelectors,
) {
    for node in nodes {
        match node {
            CssNode::Style(style_rule) => {
                let normalized = style_rule.selector.trim().to_string();
                if seen.contains_key(&normalized) {
                    diagnostics.push(
                        Diagnostic::new(
                            rule.name(),
                            format!("Unexpected duplicate selector \"{}\"", normalized),
                        )
                        .severity(rule.default_severity())
                        .span(Span::new(style_rule.span.offset, style_rule.span.length)),
                    );
                } else {
                    seen.insert(normalized, ());
                }
            }
            CssNode::AtRule(at_rule) => {
                collect_selectors(&at_rule.children, seen, diagnostics, rule);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn make_context() -> RuleContext<'static> {
        RuleContext {
            file_path: "test.css",
            source: "",
            syntax: Syntax::Css,
        }
    }

    #[test]
    fn reports_duplicate_selectors() {
        let rule = NoDuplicateSelectors;
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: ".foo".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(6, 10),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(0, 18),
            }),
            CssNode::Style(StyleRule {
                selector: ".bar".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "blue".to_string(),
                    span: ParserSpan::new(25, 11),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(19, 19),
            }),
            CssNode::Style(StyleRule {
                selector: ".foo".to_string(),
                declarations: vec![Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(45, 14),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(39, 22),
            }),
        ];
        let diags = rule.check_root(&nodes, &make_context());
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "Unexpected duplicate selector \".foo\""
        );
    }

    #[test]
    fn ignores_unique_selectors() {
        let rule = NoDuplicateSelectors;
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: ".foo".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 10),
            }),
            CssNode::Style(StyleRule {
                selector: ".bar".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(11, 10),
            }),
        ];
        let diags = rule.check_root(&nodes, &make_context());
        assert!(diags.is_empty());
    }
}
