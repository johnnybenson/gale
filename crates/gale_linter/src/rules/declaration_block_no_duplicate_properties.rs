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

        // Stylelint skips rules whose selector contains SCSS interpolation
        // (`#{...}`) via `isStandardSyntaxRule`. Match that behavior.
        if matches!(
            ctx.syntax,
            gale_css_parser::Syntax::Scss | gale_css_parser::Syntax::Sass
        ) && rule.selector.contains("#{")
        {
            return vec![];
        }

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

        let is_preprocessor = matches!(
            ctx.syntax,
            gale_css_parser::Syntax::Scss | gale_css_parser::Syntax::Sass | gale_css_parser::Syntax::Less
        );

        // Track seen properties as (prefix, unprefixed_name) to correctly
        // handle vendor-prefixed properties. `-webkit-transform` and `transform`
        // are NOT duplicates; `-webkit-transform` and `-webkit-transform` ARE.
        let mut seen: HashSet<(String, String)> = HashSet::new();
        // Track last property name+value for consecutive duplicate checks
        let mut last_prop: Option<(String, String)> = None;
        let mut diagnostics = Vec::new();

        for decl in &rule.declarations {
            // Skip properties with SCSS/Less interpolation — we can't resolve the
            // actual name, so duplicate detection would produce false positives.
            if is_preprocessor && decl.property.contains("#{") {
                last_prop = Some((
                    decl.property.to_ascii_lowercase(),
                    decl.value.to_ascii_lowercase(),
                ));
                continue;
            }
            let name = decl.property.to_ascii_lowercase();
            let value = decl.value.to_ascii_lowercase();

            let (prefix, unprefixed) = split_vendor_prefix_from_prop(&name);
            let key = (prefix.to_string(), unprefixed.to_string());

            if !seen.insert(key) {
                let is_consecutive = last_prop
                    .as_ref()
                    .map(|(prev_name, _)| prev_name == &name)
                    .unwrap_or(false);

                let has_different_value = last_prop
                    .as_ref()
                    .map(|(_, prev_val)| prev_val != &value)
                    .unwrap_or(false);

                let should_ignore = if is_consecutive {
                    if ignore_consecutive {
                        // Any consecutive duplicate is ignored.
                        true
                    } else if ignore_consecutive_diff_values {
                        has_different_value
                    } else if ignore_consecutive_diff_syntaxes {
                        let prev_value = last_prop.as_ref().map(|(_, v)| v.as_str()).unwrap_or("");
                        has_different_syntax(prev_value, &value)
                    } else {
                        // Default: always allow consecutive duplicates with
                        // different values (common fallback pattern, matching
                        // Stylelint's default behavior).
                        has_different_value
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

/// Split a property name into its vendor prefix and unprefixed name.
/// E.g., `-webkit-transform` -> (`-webkit-`, `transform`)
///       `color` -> (``, `color`)
fn split_vendor_prefix_from_prop(prop: &str) -> (&str, &str) {
    static PREFIXES: &[&str] = &["-webkit-", "-moz-", "-ms-", "-o-"];
    for prefix in PREFIXES {
        if prop.starts_with(prefix) {
            return (&prop[..prefix.len()], &prop[prefix.len()..]);
        }
    }
    ("", prop)
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
                    value: "red".to_string(),
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

    #[test]
    fn allows_vendor_prefixed_property_with_unprefixed() {
        // -webkit-user-select + user-select is a vendor fallback, not a duplicate
        let rule = DeclarationBlockNoDuplicateProperties;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "-webkit-user-select".to_string(),
                    value: "none".to_string(),
                    span: ParserSpan::new(4, 24),
                    important: false,
                },
                Declaration {
                    property: "user-select".to_string(),
                    value: "none".to_string(),
                    span: ParserSpan::new(30, 17),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, 50),
        });
        let diags = rule.check(&node, &make_context());
        assert!(diags.is_empty(), "vendor prefix + unprefixed should not be flagged");
    }

    #[test]
    fn allows_different_vendor_prefixes() {
        // -webkit-transform + -moz-transform are different vendor prefixes, not duplicates
        let rule = DeclarationBlockNoDuplicateProperties;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "-webkit-transform".to_string(),
                    value: "scale(2)".to_string(),
                    span: ParserSpan::new(4, 25),
                    important: false,
                },
                Declaration {
                    property: "-moz-transform".to_string(),
                    value: "scale(2)".to_string(),
                    span: ParserSpan::new(30, 22),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, 55),
        });
        let diags = rule.check(&node, &make_context());
        assert!(diags.is_empty(), "different vendor prefixes should not be flagged");
    }

    #[test]
    fn flags_same_vendor_prefix_duplicate() {
        // -webkit-transform + -webkit-transform IS a duplicate
        let rule = DeclarationBlockNoDuplicateProperties;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "-webkit-transform".to_string(),
                    value: "scale(2)".to_string(),
                    span: ParserSpan::new(4, 25),
                    important: false,
                },
                Declaration {
                    property: "-webkit-transform".to_string(),
                    value: "scale(2)".to_string(),
                    span: ParserSpan::new(30, 25),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, 58),
        });
        let diags = rule.check(&node, &make_context());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn allows_consecutive_duplicates_with_different_values() {
        // display: -webkit-flex; display: flex; is a common fallback pattern
        let rule = DeclarationBlockNoDuplicateProperties;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "display".to_string(),
                    value: "-webkit-flex".to_string(),
                    span: ParserSpan::new(4, 22),
                    important: false,
                },
                Declaration {
                    property: "display".to_string(),
                    value: "flex".to_string(),
                    span: ParserSpan::new(28, 13),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, 44),
        });
        let diags = rule.check(&node, &make_context());
        assert!(diags.is_empty(), "consecutive duplicates with different values should be allowed");
    }

    #[test]
    fn flags_non_consecutive_duplicates_with_different_values() {
        // color: red; display: block; color: blue; — non-consecutive, should flag
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
    }
}
