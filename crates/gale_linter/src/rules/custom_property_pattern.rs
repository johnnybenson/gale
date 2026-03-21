use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Enforce a naming pattern for custom properties (CSS variables).
///
/// Equivalent to Stylelint's `custom-property-pattern` rule.
/// Default pattern: kebab-case (`^([a-z][a-z0-9]*)(-[a-z0-9]+)*$`).
pub struct CustomPropertyPattern;

impl Rule for CustomPropertyPattern {
    fn name(&self) -> &'static str {
        "custom-property-pattern"
    }

    fn description(&self) -> &'static str {
        "Specify a pattern for custom properties"
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
            if let Some(name) = decl.property.strip_prefix("--")
                && !is_kebab_case(name)
            {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected custom property \"--{name}\" to match kebab-case pattern"
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

/// Matches `^([a-z][a-z0-9]*)(-[a-z0-9]+)*$`
fn is_kebab_case(name: &str) -> bool {
    let chars: Vec<char> = name.chars().collect();
    if chars.is_empty() {
        return false;
    }
    if !chars[0].is_ascii_lowercase() {
        return false;
    }

    let mut i = 1;
    while i < chars.len() {
        if chars[i] == '-' {
            i += 1;
            if i >= chars.len() || !(chars[i].is_ascii_lowercase() || chars[i].is_ascii_digit()) {
                return false;
            }
        } else if chars[i].is_ascii_lowercase() || chars[i].is_ascii_digit() {
            // ok
        } else {
            return false;
        }
        i += 1;
    }
    true
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

    fn style_with_property(prop: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: ":root".to_string(),
            declarations: vec![Declaration {
                property: prop.to_string(),
                value: "#fff".to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_non_kebab_custom_property() {
        let d = CustomPropertyPattern.check(&style_with_property("--myColor"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("myColor"));
    }

    #[test]
    fn allows_kebab_case_custom_property() {
        let d = CustomPropertyPattern.check(&style_with_property("--my-color"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn ignores_regular_properties() {
        let d = CustomPropertyPattern.check(&style_with_property("color"), &ctx());
        assert!(d.is_empty());
    }
}
