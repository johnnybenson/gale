use std::collections::HashSet;

use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow duplicate selectors within keyframe blocks.
///
/// Equivalent to Stylelint's `keyframe-block-no-duplicate-selectors` rule.
///
/// Checks `@keyframes` at-rules for child `Style` nodes that share the same
/// selector (e.g. two `from` or two `50%` blocks).
pub struct KeyframeBlockNoDuplicateSelectors;

impl Rule for KeyframeBlockNoDuplicateSelectors {
    fn name(&self) -> &'static str {
        "keyframe-block-no-duplicate-selectors"
    }

    fn description(&self) -> &'static str {
        "Disallow duplicate selectors within keyframe blocks"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _context: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::AtRule(at_rule) = node else {
            return vec![];
        };

        if at_rule.name != "keyframes" {
            return vec![];
        }

        let mut seen = HashSet::new();
        let mut diagnostics = Vec::new();

        for child in &at_rule.children {
            if let CssNode::Style(style_rule) = child {
                let selector = style_rule.selector.trim().to_ascii_lowercase();
                if !seen.insert(selector.clone()) {
                    diagnostics.push(
                        Diagnostic::new(
                            self.name(),
                            format!("Unexpected duplicate keyframe selector \"{}\"", selector),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(style_rule.span.offset, style_rule.span.length)),
                    );
                }
            }
        }

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{AtRule, Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn make_context() -> RuleContext<'static> {
        RuleContext {
            file_path: "test.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        }
    }

    #[test]
    fn reports_duplicate_keyframe_selectors() {
        let rule = KeyframeBlockNoDuplicateSelectors;
        let node = CssNode::AtRule(AtRule {
            name: "keyframes".to_string(),
            params: "fade".to_string(),
            span: ParserSpan::new(0, 80),
            children: vec![
                CssNode::Style(StyleRule {
                    selector: "from".to_string(),
                    declarations: vec![Declaration {
                        property: "opacity".to_string(),
                        value: "0".to_string(),
                        span: ParserSpan::new(25, 10),
                        important: false,
                    }],
                    children: vec![],
                    span: ParserSpan::new(19, 20),
                }),
                CssNode::Style(StyleRule {
                    selector: "to".to_string(),
                    declarations: vec![Declaration {
                        property: "opacity".to_string(),
                        value: "1".to_string(),
                        span: ParserSpan::new(48, 10),
                        important: false,
                    }],
                    children: vec![],
                    span: ParserSpan::new(40, 20),
                }),
                CssNode::Style(StyleRule {
                    selector: "from".to_string(),
                    declarations: vec![Declaration {
                        property: "opacity".to_string(),
                        value: "0.5".to_string(),
                        span: ParserSpan::new(68, 12),
                        important: false,
                    }],
                    children: vec![],
                    span: ParserSpan::new(61, 22),
                }),
            ],
        });
        let diags = rule.check(&node, &make_context());
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "Unexpected duplicate keyframe selector \"from\""
        );
    }

    #[test]
    fn ignores_unique_keyframe_selectors() {
        let rule = KeyframeBlockNoDuplicateSelectors;
        let node = CssNode::AtRule(AtRule {
            name: "keyframes".to_string(),
            params: "fade".to_string(),
            span: ParserSpan::new(0, 60),
            children: vec![
                CssNode::Style(StyleRule {
                    selector: "from".to_string(),
                    declarations: vec![],
                    children: vec![],
                    span: ParserSpan::new(19, 15),
                }),
                CssNode::Style(StyleRule {
                    selector: "to".to_string(),
                    declarations: vec![],
                    children: vec![],
                    span: ParserSpan::new(35, 13),
                }),
            ],
        });
        let diags = rule.check(&node, &make_context());
        assert!(diags.is_empty());
    }

    #[test]
    fn ignores_non_keyframes_at_rules() {
        let rule = KeyframeBlockNoDuplicateSelectors;
        let node = CssNode::AtRule(AtRule {
            name: "media".to_string(),
            params: "(min-width: 768px)".to_string(),
            span: ParserSpan::new(0, 40),
            children: vec![],
        });
        let diags = rule.check(&node, &make_context());
        assert!(diags.is_empty());
    }
}
