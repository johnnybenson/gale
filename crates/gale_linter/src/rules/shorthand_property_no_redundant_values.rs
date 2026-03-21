use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports shorthand property values that contain redundant parts
/// (e.g. `margin: 1px 1px 1px 1px` → `margin: 1px`).
///
/// Equivalent to Stylelint's `shorthand-property-no-redundant-values` rule.
pub struct ShorthandPropertyNoRedundantValues;

const SHORTHAND_PROPERTIES: &[&str] = &[
    "margin",
    "padding",
    "border-color",
    "border-style",
    "border-width",
    "border-radius",
    "gap",
    "grid-gap",
    "overflow",
    "inset",
];

impl Rule for ShorthandPropertyNoRedundantValues {
    fn name(&self) -> &'static str {
        "shorthand-property-no-redundant-values"
    }

    fn description(&self) -> &'static str {
        "Disallow redundant values in shorthand properties"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let mut diags = Vec::new();
        for decl in &rule.declarations {
            let prop = decl.property.to_ascii_lowercase();
            if !SHORTHAND_PROPERTIES.contains(&prop.as_str()) {
                continue;
            }
            let parts: Vec<&str> = decl.value.split_whitespace().collect();
            if let Some(shortened) = shorten(&parts) {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected \"{}\" instead of \"{}\"",
                            shortened,
                            decl.value
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

/// Try to shorten redundant values. Returns Some(shortened) if redundant.
fn shorten(parts: &[&str]) -> Option<String> {
    match parts.len() {
        4 => {
            let (top, right, bottom, left) = (parts[0], parts[1], parts[2], parts[3]);
            if top == right && right == bottom && bottom == left {
                // 1px 1px 1px 1px → 1px
                Some(top.to_string())
            } else if top == bottom && right == left {
                // 1px 2px 1px 2px → 1px 2px
                Some(format!("{top} {right}"))
            } else if right == left {
                // 1px 2px 3px 2px → 1px 2px 3px
                Some(format!("{top} {right} {bottom}"))
            } else {
                None
            }
        }
        3 => {
            let (top, right, bottom) = (parts[0], parts[1], parts[2]);
            if top == right && right == bottom {
                Some(top.to_string())
            } else if top == bottom {
                Some(format!("{top} {right}"))
            } else {
                None
            }
        }
        2 => {
            if parts[0] == parts[1] {
                Some(parts[0].to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css }
    }

    fn style_decl(prop: &str, val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: prop.to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_four_identical_values() {
        let d = ShorthandPropertyNoRedundantValues.check(
            &style_decl("margin", "1px 1px 1px 1px"),
            &ctx(),
        );
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("1px"));
    }

    #[test]
    fn reports_two_identical_values() {
        let d = ShorthandPropertyNoRedundantValues.check(
            &style_decl("padding", "10px 10px"),
            &ctx(),
        );
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn allows_non_redundant_values() {
        assert!(
            ShorthandPropertyNoRedundantValues
                .check(&style_decl("margin", "1px 2px 3px 4px"), &ctx())
                .is_empty()
        );
        assert!(
            ShorthandPropertyNoRedundantValues
                .check(&style_decl("margin", "1px"), &ctx())
                .is_empty()
        );
    }
}
