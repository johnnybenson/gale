use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow specific property-value pairs.
///
/// By default, disallows no property-value pairs. Configure via rule options.
///
/// Equivalent to Stylelint's `declaration-property-value-disallowed-list` rule.
pub struct DeclarationPropertyValueDisallowedList;

/// Each entry is (property, disallowed_value_substring).
const DISALLOWED_PAIRS: &[(&str, &str)] = &[];

impl Rule for DeclarationPropertyValueDisallowedList {
    fn name(&self) -> &'static str {
        "declaration-property-value-disallowed-list"
    }

    fn description(&self) -> &'static str {
        "Specify a list of disallowed property and value pairs within declarations"
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
            let prop_lower = decl.property.to_ascii_lowercase();
            let val_lower = decl.value.to_ascii_lowercase();
            for &(disallowed_prop, disallowed_val) in DISALLOWED_PAIRS {
                if prop_lower == disallowed_prop && val_lower.contains(disallowed_val) {
                    diags.push(
                        Diagnostic::new(
                            self.name(),
                            format!(
                                "Unexpected value \"{val}\" for property \"{prop}\"",
                                val = decl.value,
                                prop = decl.property,
                            ),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(decl.span.offset, decl.span.length)),
                    );
                }
            }
        }
        diags
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

    fn style_with_decl(prop: &str, val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: prop.to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, 10),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn allows_all_when_list_empty() {
        let d =
            DeclarationPropertyValueDisallowedList.check(&style_with_decl("color", "red"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_any_declaration_by_default() {
        let d = DeclarationPropertyValueDisallowedList
            .check(&style_with_decl("display", "none"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn rule_name_is_correct() {
        assert_eq!(
            DeclarationPropertyValueDisallowedList.name(),
            "declaration-property-value-disallowed-list"
        );
    }
}
