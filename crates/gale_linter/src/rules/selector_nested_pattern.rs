use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Nested selectors must start with `&`.
///
/// Equivalent to Stylelint's `selector-nested-pattern` rule with pattern "^&".
pub struct SelectorNestedPattern;

impl Rule for SelectorNestedPattern {
    fn name(&self) -> &'static str {
        "selector-nested-pattern"
    }

    fn description(&self) -> &'static str {
        "Specify a pattern for the selectors of rules nested within rules"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let mut diags = Vec::new();
        check_nested_selectors(self, rule, &mut diags);
        diags
    }
}

fn check_nested_selectors(
    rule: &SelectorNestedPattern,
    style: &gale_css_parser::StyleRule,
    diags: &mut Vec<Diagnostic>,
) {
    for child in &style.children {
        let selector = child.selector.trim();
        if !selector.starts_with('&') {
            diags.push(
                Diagnostic::new(
                    rule.name(),
                    format!("Expected nested selector \"{selector}\" to start with \"&\""),
                )
                .severity(rule.default_severity())
                .span(Span::new(child.span.offset, child.span.length)),
            );
        }
        // Recurse into deeper nesting
        check_nested_selectors(rule, child, diags);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        }
    }

    fn make_decl() -> Declaration {
        Declaration {
            property: "color".to_string(),
            value: "red".to_string(),
            span: ParserSpan::new(0, 0),
            important: false,
        }
    }

    #[test]
    fn reports_nested_without_ampersand() {
        let node = CssNode::Style(StyleRule {
            selector: ".parent".to_string(),
            declarations: vec![make_decl()],
            children: vec![StyleRule {
                selector: ".child".to_string(),
                declarations: vec![make_decl()],
                span: ParserSpan::new(20, 15),
                ..Default::default()
            }],
            span: ParserSpan::new(0, 40),
        
            nested_at_rules: Vec::new(),
});
        let d = SelectorNestedPattern.check(&node, &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains(".child"));
    }

    #[test]
    fn allows_nested_with_ampersand() {
        let node = CssNode::Style(StyleRule {
            selector: ".parent".to_string(),
            declarations: vec![make_decl()],
            children: vec![StyleRule {
                selector: "&:hover".to_string(),
                declarations: vec![make_decl()],
                span: ParserSpan::new(20, 15),
                ..Default::default()
            }],
            span: ParserSpan::new(0, 40),
        
            nested_at_rules: Vec::new(),
});
        let d = SelectorNestedPattern.check(&node, &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn no_diagnostics_for_no_children() {
        let node = CssNode::Style(StyleRule {
            selector: ".parent".to_string(),
            declarations: vec![make_decl()],
span: ParserSpan::new(0, 20),
            ..Default::default()
});
        let d = SelectorNestedPattern.check(&node, &ctx());
        assert!(d.is_empty());
    }
}
