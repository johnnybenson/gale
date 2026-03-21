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

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        // Check for ignore options
        let ignore_list: Vec<String> = ctx
            .options
            .and_then(|v| v.get("ignore"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let ignore_consecutive = ignore_list.iter().any(|s| s == "consecutive-duplicates");
        let ignore_consecutive_diff_syntaxes = ignore_list
            .iter()
            .any(|s| s == "consecutive-duplicates-with-different-syntaxes");
        let ignore_consecutive_diff_values = ignore_list
            .iter()
            .any(|s| s == "consecutive-duplicates-with-different-values");

        let mut seen = HashSet::new();
        // Track last property name+value for consecutive duplicate checks
        let mut last_prop: Option<(String, String)> = None;
        let mut diagnostics = Vec::new();

        for decl in &rule.declarations {
            let name = decl.property.to_ascii_lowercase();
            let value = decl.value.to_ascii_lowercase();

            if !seen.insert(name.clone()) {
                let is_consecutive = last_prop
                    .as_ref()
                    .map(|(prev_name, _)| prev_name == &name)
                    .unwrap_or(false);

                let should_ignore = if is_consecutive {
                    if ignore_consecutive {
                        // Any consecutive duplicate is ignored.
                        true
                    } else if ignore_consecutive_diff_values {
                        let prev_value = last_prop.as_ref().map(|(_, v)| v.as_str()).unwrap_or("");
                        prev_value != value
                    } else if ignore_consecutive_diff_syntaxes {
                        let prev_value = last_prop.as_ref().map(|(_, v)| v.as_str()).unwrap_or("");
                        has_different_syntax(prev_value, &value)
                    } else {
                        // Default: always allow consecutive duplicates where one
                        // value has a vendor prefix (common fallback pattern).
                        let prev_value = last_prop.as_ref().map(|(_, v)| v.as_str()).unwrap_or("");
                        has_vendor_prefix_fallback(prev_value, &value)
                    }
                } else {
                    false
                };

                if !should_ignore {
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

            last_prop = Some((name, value));
        }

        diagnostics
    }
}

/// Check if two values have different CSS "syntaxes" (one uses a vendor
/// prefix or a fundamentally different value form).
/// Check if two consecutive values represent a vendor-prefix fallback pattern.
/// E.g., `text-align: inherit` followed by `text-align: -webkit-match-parent`.
fn has_vendor_prefix_fallback(a: &str, b: &str) -> bool {
    has_vendor_prefix(a) || has_vendor_prefix(b)
}

fn has_different_syntax(a: &str, b: &str) -> bool {
    let a_has_prefix = has_vendor_prefix(a);
    let b_has_prefix = has_vendor_prefix(b);
    // If one is vendor-prefixed and the other is not, they have different syntaxes
    if a_has_prefix != b_has_prefix {
        return true;
    }
    // If they use different vendor prefixes
    if a_has_prefix && b_has_prefix {
        let a_prefix = extract_vendor_prefix(a);
        let b_prefix = extract_vendor_prefix(b);
        if a_prefix != b_prefix {
            return true;
        }
    }
    false
}

fn has_vendor_prefix(value: &str) -> bool {
    let v = value.trim();
    v.contains("-webkit-") || v.contains("-moz-") || v.contains("-ms-") || v.contains("-o-")
}

fn extract_vendor_prefix(value: &str) -> &str {
    if value.contains("-webkit-") {
        "-webkit-"
    } else if value.contains("-moz-") {
        "-moz-"
    } else if value.contains("-ms-") {
        "-ms-"
    } else if value.contains("-o-") {
        "-o-"
    } else {
        ""
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
            options: None,
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
        assert_eq!(diags[0].message, "Unexpected duplicate property \"color\"");
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
