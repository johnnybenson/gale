use std::collections::HashMap;

use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Enforce a specific ordering of properties within declaration blocks.
///
/// Equivalent to stylelint-order's `order/properties-order` rule.
///
/// The rule accepts a JSON array as its primary option. Each element can be:
/// - A string: a property name (e.g. `"position"`)
/// - An object with a `"properties"` array: a group of property names
///   (e.g. `{ "properties": ["display", "flex-direction"] }`)
///
/// Properties not listed in the order are ignored and may appear anywhere.
pub struct OrderPropertiesOrder;

impl Rule for OrderPropertiesOrder {
    fn name(&self) -> &'static str {
        "order/properties-order"
    }

    fn description(&self) -> &'static str {
        "Enforce a specific ordering of properties within declaration blocks"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        // Parse the expected property order from options.
        let order_map = match build_order_map(ctx.options) {
            Some(map) => map,
            None => return vec![], // no config → nothing to check
        };

        let mut diagnostics = Vec::new();
        let mut last_seen_index: Option<usize> = None;
        let mut last_seen_property: Option<String> = None;

        for decl in &rule.declarations {
            let prop = &decl.property;

            // Skip SCSS variables ($var) and custom properties (--var)
            if prop.starts_with('$') || prop.starts_with("--") {
                continue;
            }

            let prop_lower = prop.to_ascii_lowercase();

            if let Some(&expected_idx) = order_map.get(prop_lower.as_str()) {
                if let Some(prev_idx) = last_seen_index {
                    if expected_idx < prev_idx {
                        let prev_name =
                            last_seen_property.as_deref().unwrap_or("unknown");
                        diagnostics.push(
                            Diagnostic::new(
                                self.name(),
                                format!(
                                    "Expected \"{prop}\" to come before \"{prev_name}\""
                                ),
                            )
                            .severity(self.default_severity())
                            .span(Span::new(decl.span.offset, decl.span.length)),
                        );
                    }
                }
                last_seen_index = Some(expected_idx);
                last_seen_property = Some(prop.clone());
            }
            // Properties not in the order list are ignored
        }

        diagnostics
    }
}

/// Build a map from property name (lowercase) to its expected position index.
///
/// The options value should be a JSON array where each element is either:
/// - A string (property name)
/// - An object with a `"properties"` key containing an array of strings
fn build_order_map(options: Option<&serde_json::Value>) -> Option<HashMap<&str, usize>> {
    let arr = options?.as_array()?;

    let mut map = HashMap::new();
    let mut idx = 0usize;

    for item in arr {
        match item {
            serde_json::Value::String(s) => {
                // Direct string: a single property name
                map.insert(s.as_str(), idx);
                idx += 1;
            }
            serde_json::Value::Object(obj) => {
                // Object with "properties" array
                if let Some(props) = obj.get("properties").and_then(|v| v.as_array()) {
                    for prop in props {
                        if let Some(s) = prop.as_str() {
                            map.insert(s, idx);
                            idx += 1;
                        }
                    }
                }
            }
            _ => {
                // Skip unknown items
            }
        }
    }

    if map.is_empty() {
        None
    } else {
        Some(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx_with_options(options: &serde_json::Value) -> RuleContext<'_> {
        RuleContext {
            file_path: "test.css",
            source: "",
            syntax: Syntax::Css,
            options: Some(options),
        }
    }

    fn ctx_no_options() -> RuleContext<'static> {
        RuleContext {
            file_path: "test.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        }
    }

    fn make_decl(property: &str, value: &str, offset: usize, length: usize) -> Declaration {
        Declaration {
            property: property.to_string(),
            value: value.to_string(),
            span: ParserSpan::new(offset, length),
            important: false,
        }
    }

    #[test]
    fn no_options_no_diagnostics() {
        let rule = OrderPropertiesOrder;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                make_decl("display", "block", 4, 14),
                make_decl("position", "relative", 19, 19),
            ],
            children: vec![],
            span: ParserSpan::new(0, 40),
        });
        let diags = rule.check(&node, &ctx_no_options());
        assert!(diags.is_empty());
    }

    #[test]
    fn simple_array_correct_order() {
        let rule = OrderPropertiesOrder;
        let options =
            serde_json::json!(["position", "top", "right", "display", "width"]);
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                make_decl("position", "relative", 4, 19),
                make_decl("top", "0", 24, 6),
                make_decl("display", "block", 31, 14),
                make_decl("width", "100%", 46, 12),
            ],
            children: vec![],
            span: ParserSpan::new(0, 60),
        });
        let diags = rule.check(&node, &ctx_with_options(&options));
        assert!(diags.is_empty());
    }

    #[test]
    fn simple_array_wrong_order() {
        let rule = OrderPropertiesOrder;
        let options =
            serde_json::json!(["position", "top", "right", "display", "width"]);
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                make_decl("display", "block", 4, 14),
                make_decl("position", "relative", 19, 19),
            ],
            children: vec![],
            span: ParserSpan::new(0, 40),
        });
        let diags = rule.check(&node, &ctx_with_options(&options));
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("position"));
        assert!(diags[0].message.contains("display"));
    }

    #[test]
    fn grouped_objects_correct_order() {
        let rule = OrderPropertiesOrder;
        let options = serde_json::json!([
            { "properties": ["position", "top", "right", "bottom", "left"] },
            { "properties": ["display", "flex-direction"] },
            { "properties": ["width", "height"] }
        ]);
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                make_decl("position", "absolute", 4, 19),
                make_decl("top", "0", 24, 6),
                make_decl("display", "flex", 31, 13),
                make_decl("width", "100px", 45, 13),
            ],
            children: vec![],
            span: ParserSpan::new(0, 60),
        });
        let diags = rule.check(&node, &ctx_with_options(&options));
        assert!(diags.is_empty());
    }

    #[test]
    fn grouped_objects_wrong_order() {
        let rule = OrderPropertiesOrder;
        let options = serde_json::json!([
            { "properties": ["position", "top", "right", "bottom", "left"] },
            { "properties": ["display", "flex-direction"] },
            { "properties": ["width", "height"] }
        ]);
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                make_decl("width", "100px", 4, 13),
                make_decl("position", "absolute", 18, 19),
            ],
            children: vec![],
            span: ParserSpan::new(0, 40),
        });
        let diags = rule.check(&node, &ctx_with_options(&options));
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("position"));
    }

    #[test]
    fn mixed_strings_and_objects() {
        let rule = OrderPropertiesOrder;
        let options = serde_json::json!([
            "position",
            { "properties": ["display", "flex"] },
            "color"
        ]);
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                make_decl("position", "relative", 4, 19),
                make_decl("display", "flex", 24, 13),
                make_decl("color", "red", 38, 10),
            ],
            children: vec![],
            span: ParserSpan::new(0, 50),
        });
        let diags = rule.check(&node, &ctx_with_options(&options));
        assert!(diags.is_empty());
    }

    #[test]
    fn unknown_properties_are_ignored() {
        let rule = OrderPropertiesOrder;
        let options =
            serde_json::json!(["position", "display", "width"]);
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                make_decl("position", "relative", 4, 19),
                // "unknown-prop" not in the list => ignored
                make_decl("unknown-prop", "foo", 24, 18),
                make_decl("display", "block", 43, 14),
            ],
            children: vec![],
            span: ParserSpan::new(0, 60),
        });
        let diags = rule.check(&node, &ctx_with_options(&options));
        assert!(diags.is_empty());
    }

    #[test]
    fn skips_scss_variables() {
        let rule = OrderPropertiesOrder;
        let options =
            serde_json::json!(["position", "display", "width"]);
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                make_decl("display", "block", 4, 14),
                make_decl("$my-var", "10px", 19, 14),
                make_decl("position", "relative", 34, 19),
            ],
            children: vec![],
            span: ParserSpan::new(0, 55),
        });
        let diags = rule.check(&node, &ctx_with_options(&options));
        // "position" should come before "display" => 1 diagnostic
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn skips_custom_properties() {
        let rule = OrderPropertiesOrder;
        let options =
            serde_json::json!(["position", "display", "width"]);
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                make_decl("position", "relative", 4, 19),
                make_decl("--my-var", "10px", 24, 16),
                make_decl("display", "block", 41, 14),
            ],
            children: vec![],
            span: ParserSpan::new(0, 60),
        });
        let diags = rule.check(&node, &ctx_with_options(&options));
        assert!(diags.is_empty());
    }
}
