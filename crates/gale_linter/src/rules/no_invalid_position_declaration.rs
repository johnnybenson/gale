use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports declarations that appear directly inside at-rules that don't
/// support them (like `@media`, `@supports`).
///
/// A declaration inside `@media` without a selector wrapper is invalid CSS.
///
/// Equivalent to Stylelint's `no-invalid-position-declaration` rule.
pub struct NoInvalidPositionDeclaration;

/// At-rules that act as conditional wrappers and should not contain
/// declarations directly (only nested rules).
const WRAPPER_AT_RULES: &[&str] = &[
    "container",
    "document",
    "layer",
    "media",
    "scope",
    "starting-style",
    "supports",
];

impl Rule for NoInvalidPositionDeclaration {
    fn name(&self) -> &'static str {
        "no-invalid-position-declaration"
    }

    fn description(&self) -> &'static str {
        "Disallow invalid position declarations"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::AtRule(at) = node else {
            return vec![];
        };

        if !WRAPPER_AT_RULES.contains(&at.name.as_str()) {
            return vec![];
        }

        let mut diags = Vec::new();
        for child in &at.children {
            if let CssNode::Declaration(decl) = child {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Unexpected declaration \"{}\" directly inside @{}",
                            decl.property, at.name
                        ),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(at.span.offset, at.span.length)),
                );
            }
        }
        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{AtRule, Declaration, Span as ParserSpan, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css, options: None }
    }

    #[test]
    fn reports_declaration_inside_media() {
        let node = CssNode::AtRule(AtRule {
            name: "media".to_string(),
            params: "(min-width: 768px)".to_string(),
            span: ParserSpan::new(0, 0),
            children: vec![CssNode::Declaration(Declaration {
                property: "color".to_string(),
                value: "red".to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            })],
        });
        let d = NoInvalidPositionDeclaration.check(&node, &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("color"));
        assert!(d[0].message.contains("@media"));
    }

    #[test]
    fn allows_style_rules_inside_media() {
        let node = CssNode::AtRule(AtRule {
            name: "media".to_string(),
            params: "(min-width: 768px)".to_string(),
            span: ParserSpan::new(0, 0),
            children: vec![CssNode::Style(gale_css_parser::StyleRule {
                selector: "a".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(0, 0),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(0, 0),
            })],
        });
        let d = NoInvalidPositionDeclaration.check(&node, &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_declarations_inside_font_face() {
        let node = CssNode::AtRule(AtRule {
            name: "font-face".to_string(),
            params: String::new(),
            span: ParserSpan::new(0, 0),
            children: vec![CssNode::Declaration(Declaration {
                property: "font-family".to_string(),
                value: "MyFont".to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            })],
        });
        let d = NoInvalidPositionDeclaration.check(&node, &ctx());
        assert!(d.is_empty());
    }
}
