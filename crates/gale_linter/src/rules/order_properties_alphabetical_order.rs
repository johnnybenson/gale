use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Enforce alphabetical ordering of properties within declaration blocks.
///
/// Equivalent to stylelint-order's `order/properties-alphabetical-order` rule.
pub struct OrderPropertiesAlphabeticalOrder;

impl Rule for OrderPropertiesAlphabeticalOrder {
    fn name(&self) -> &'static str {
        "order/properties-alphabetical-order"
    }

    fn description(&self) -> &'static str {
        "Require properties within declaration blocks to be in alphabetical order"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _context: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        let mut diagnostics = Vec::new();
        let mut prev_property: Option<String> = None;

        for decl in &rule.declarations {
            let prop = &decl.property;

            // Skip SCSS variables ($var)
            if prop.starts_with('$') {
                continue;
            }

            // Skip custom properties (--var)
            if prop.starts_with("--") {
                continue;
            }

            let prop_lower = prop.to_ascii_lowercase();

            if let Some(ref prev) = prev_property
                && prop_lower < *prev
            {
                diagnostics.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected \"{prop}\" to come before \"{}\"",
                            find_original_prev(rule, prev)
                        ),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(decl.span.offset, decl.span.length)),
                );
            }

            prev_property = Some(prop_lower);
        }

        diagnostics
    }
}

/// Find the original (non-lowered) property name that corresponds to the
/// lowercased previous property, for a nicer diagnostic message.
fn find_original_prev(rule: &gale_css_parser::StyleRule, lower: &str) -> String {
    for decl in rule.declarations.iter().rev() {
        if decl.property.to_ascii_lowercase() == *lower {
            return decl.property.clone();
        }
    }
    lower.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "test.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        }
    }

    #[test]
    fn accepts_alphabetical_order() {
        let rule = OrderPropertiesAlphabeticalOrder;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(4, 10),
                    important: false,
                },
                Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(15, 14),
                    important: false,
                },
                Declaration {
                    property: "font-size".to_string(),
                    value: "12px".to_string(),
                    span: ParserSpan::new(30, 15),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, 50),
        });
        let diags = rule.check(&node, &ctx());
        assert!(diags.is_empty());
    }

    #[test]
    fn reports_non_alphabetical_order() {
        let rule = OrderPropertiesAlphabeticalOrder;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(4, 14),
                    important: false,
                },
                Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(19, 10),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, 35),
        });
        let diags = rule.check(&node, &ctx());
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("color"));
        assert!(diags[0].message.contains("display"));
    }

    #[test]
    fn skips_scss_variables() {
        let rule = OrderPropertiesAlphabeticalOrder;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(4, 14),
                    important: false,
                },
                Declaration {
                    property: "$my-var".to_string(),
                    value: "10px".to_string(),
                    span: ParserSpan::new(19, 14),
                    important: false,
                },
                Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(34, 10),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, 50),
        });
        let diags = rule.check(&node, &ctx());
        // "color" comes after "display" which is out of order, but $my-var is skipped
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn case_insensitive_comparison() {
        let rule = OrderPropertiesAlphabeticalOrder;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "Color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(4, 10),
                    important: false,
                },
                Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(15, 14),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, 35),
        });
        let diags = rule.check(&node, &ctx());
        assert!(diags.is_empty());
    }

    #[test]
    fn skips_custom_properties() {
        let rule = OrderPropertiesAlphabeticalOrder;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(4, 14),
                    important: false,
                },
                Declaration {
                    property: "--my-var".to_string(),
                    value: "10px".to_string(),
                    span: ParserSpan::new(19, 16),
                    important: false,
                },
                Declaration {
                    property: "font-size".to_string(),
                    value: "12px".to_string(),
                    span: ParserSpan::new(36, 15),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, 55),
        });
        let diags = rule.check(&node, &ctx());
        assert!(diags.is_empty());
    }
}
