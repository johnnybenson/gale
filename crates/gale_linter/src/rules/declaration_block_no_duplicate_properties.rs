use std::collections::HashSet;

use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow duplicate properties within declaration blocks.
///
/// Equivalent to Stylelint's `declaration-block-no-duplicate-properties` rule.
pub struct DeclarationBlockNoDuplicateProperties;

impl Rule for DeclarationBlockNoDuplicateProperties {
    fn name(&self) -> &'static str {
        "declaration-block-no-duplicate-properties"
    }

    fn description(&self) -> &'static str {
        "Disallow duplicate properties within declaration blocks"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _context: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        let mut seen = HashSet::new();
        let mut diagnostics = Vec::new();

        for decl in &rule.declarations {
            let name = decl.property.to_ascii_lowercase();
            if !seen.insert(name.clone()) {
                diagnostics.push(
                    Diagnostic::new(
                        self.name(),
                        format!("Unexpected duplicate property \"{}\"", decl.property),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(decl.span.offset, decl.span.length)),
                );
            }
        }

        diagnostics
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
    fn reports_duplicate_properties() {
        let rule = DeclarationBlockNoDuplicateProperties;
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
                    property: "color".to_string(),
                    value: "blue".to_string(),
                    span: ParserSpan::new(30, 11),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, 45),
        });
        let diags = rule.check(&node, &make_context());
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "Unexpected duplicate property \"color\""
        );
    }

    #[test]
    fn ignores_unique_properties() {
        let rule = DeclarationBlockNoDuplicateProperties;
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
            ],
            children: vec![],
            span: ParserSpan::new(0, 30),
        });
        let diags = rule.check(&node, &make_context());
        assert!(diags.is_empty());
    }

    #[test]
    fn case_insensitive_detection() {
        let rule = DeclarationBlockNoDuplicateProperties;
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
                    property: "color".to_string(),
                    value: "blue".to_string(),
                    span: ParserSpan::new(15, 11),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, 30),
        });
        let diags = rule.check(&node, &make_context());
        assert_eq!(diags.len(), 1);
    }
}
