use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};
use regex::Regex;

use crate::rule::{Rule, RuleContext};

const DEFAULT_PATTERN: &str = "^([a-z][a-z0-9]*)(-[a-z0-9]+)*$";

/// Enforce a naming pattern for custom properties (CSS variables).
///
/// Equivalent to Stylelint's `custom-property-pattern` rule.
/// Accepts a regex string as the primary option.
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

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        // Read the user-supplied regex pattern from options, or use the default kebab-case pattern.
        // Options may be a plain string (`"^pattern$"`) or an array where the
        // first element is the pattern and the second is a secondary options
        // object (e.g. `["^pattern$", { "message": "..." }]`).
        let pattern_str = ctx.primary_option_str().unwrap_or(DEFAULT_PATTERN);

        let re = match Regex::new(pattern_str) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        // Check for custom message in secondary options
        let custom_message = ctx
            .secondary_options()
            .and_then(|v| v.get("message"))
            .and_then(|v| v.as_str());

        let mut diags = Vec::new();
        for decl in &rule.declarations {
            if let Some(name) = decl.property.strip_prefix("--") {
                // Skip names containing SCSS interpolation #{...}
                if matches!(
                    ctx.syntax,
                    gale_css_parser::Syntax::Scss | gale_css_parser::Syntax::Sass
                ) && name.contains("#{")
                {
                    continue;
                }
                if !re.is_match(name) {
                    let message = if let Some(msg) = custom_message {
                        msg.to_string()
                    } else {
                        format!(
                            "Expected custom property \"--{name}\" to match pattern \"{pattern_str}\""
                        )
                    };
                    diags.push(
                        Diagnostic::new(self.name(), message)
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

    fn ctx_with_options(opts: &serde_json::Value) -> RuleContext<'_> {
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: Some(opts),
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
            span: ParserSpan::new(0, 0),
            ..Default::default()
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

    #[test]
    fn custom_pattern() {
        let opts = serde_json::json!("^pf-(v[56]|t-)-(color|global|chart|[lcud])-.+$");
        let c = ctx_with_options(&opts);
        // Should pass: matches the custom pattern
        assert!(
            CustomPropertyPattern
                .check(&style_with_property("--pf-v5-color-primary"), &c)
                .is_empty()
        );
        // Should fail: doesn't match the custom pattern
        let d = CustomPropertyPattern.check(&style_with_property("--my-color"), &c);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn custom_pattern_in_message() {
        let opts = serde_json::json!("^pf-");
        let c = ctx_with_options(&opts);
        let d = CustomPropertyPattern.check(&style_with_property("--my-color"), &c);
        assert!(d[0].message.contains("^pf-"));
    }

    #[test]
    fn array_format_options_with_secondary() {
        // Config like: ["^sds|slds-c|kx-...", { "message": "..." }]
        let opts = serde_json::json!(["^sds|slds-c|kx-([a-z][a-z0-9]*)(-[a-z0-9]+)*$", { "message": "Custom msg" }]);
        let c = ctx_with_options(&opts);
        // _slds-c-... should match via the `slds-c` alternative
        assert!(
            CustomPropertyPattern
                .check(&style_with_property("--_slds-c-accordion-spacing"), &c)
                .is_empty(),
            "Should not flag --_slds-c-... with the SLDS pattern"
        );
        // my-color should be flagged (doesn't match the pattern)
        let d = CustomPropertyPattern.check(&style_with_property("--my-color"), &c);
        assert_eq!(d.len(), 1);
    }
}
