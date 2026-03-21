use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports when all longhand properties of a shorthand are present and could be
/// combined (e.g. margin-top + margin-right + margin-bottom + margin-left → margin).
///
/// Equivalent to Stylelint's `declaration-block-no-redundant-longhand-properties` rule.
pub struct DeclarationBlockNoRedundantLonghandProperties;

struct ShorthandMapping {
    shorthand: &'static str,
    longhands: &'static [&'static str],
}

const SHORTHAND_MAPPINGS: &[ShorthandMapping] = &[
    ShorthandMapping {
        shorthand: "margin",
        longhands: &["margin-top", "margin-right", "margin-bottom", "margin-left"],
    },
    ShorthandMapping {
        shorthand: "padding",
        longhands: &["padding-top", "padding-right", "padding-bottom", "padding-left"],
    },
    ShorthandMapping {
        shorthand: "border-color",
        longhands: &[
            "border-top-color",
            "border-right-color",
            "border-bottom-color",
            "border-left-color",
        ],
    },
    ShorthandMapping {
        shorthand: "border-style",
        longhands: &[
            "border-top-style",
            "border-right-style",
            "border-bottom-style",
            "border-left-style",
        ],
    },
    ShorthandMapping {
        shorthand: "border-width",
        longhands: &[
            "border-top-width",
            "border-right-width",
            "border-bottom-width",
            "border-left-width",
        ],
    },
    ShorthandMapping {
        shorthand: "overflow",
        longhands: &["overflow-x", "overflow-y"],
    },
    ShorthandMapping {
        shorthand: "outline",
        longhands: &["outline-color", "outline-style", "outline-width"],
    },
    ShorthandMapping {
        shorthand: "flex",
        longhands: &["flex-grow", "flex-shrink", "flex-basis"],
    },
    ShorthandMapping {
        shorthand: "grid-gap",
        longhands: &["grid-row-gap", "grid-column-gap"],
    },
    ShorthandMapping {
        shorthand: "gap",
        longhands: &["row-gap", "column-gap"],
    },
    ShorthandMapping {
        shorthand: "inset",
        longhands: &["top", "right", "bottom", "left"],
    },
    ShorthandMapping {
        shorthand: "transition",
        longhands: &[
            "transition-property",
            "transition-duration",
            "transition-timing-function",
            "transition-delay",
        ],
    },
    ShorthandMapping {
        shorthand: "animation",
        longhands: &[
            "animation-name",
            "animation-duration",
            "animation-timing-function",
            "animation-delay",
            "animation-iteration-count",
            "animation-direction",
            "animation-fill-mode",
            "animation-play-state",
        ],
    },
    ShorthandMapping {
        shorthand: "list-style",
        longhands: &["list-style-type", "list-style-position", "list-style-image"],
    },
    ShorthandMapping {
        shorthand: "border-radius",
        longhands: &[
            "border-top-left-radius",
            "border-top-right-radius",
            "border-bottom-right-radius",
            "border-bottom-left-radius",
        ],
    },
];

impl Rule for DeclarationBlockNoRedundantLonghandProperties {
    fn name(&self) -> &'static str {
        "declaration-block-no-redundant-longhand-properties"
    }

    fn description(&self) -> &'static str {
        "Disallow longhand properties that can be combined into one shorthand property"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        let props: Vec<String> = rule
            .declarations
            .iter()
            .map(|d| d.property.to_ascii_lowercase())
            .collect();

        let mut diags = Vec::new();

        for mapping in SHORTHAND_MAPPINGS {
            let all_present = mapping
                .longhands
                .iter()
                .all(|lh| props.contains(&lh.to_string()));
            if all_present {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected shorthand property \"{}\" instead of its longhands",
                            mapping.shorthand,
                        ),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(rule.span.offset, rule.span.length)),
                );
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
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css, options: None }
    }

    fn style_with_props(props: &[(&str, &str)]) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: props
                .iter()
                .map(|(p, v)| Declaration {
                    property: p.to_string(),
                    value: v.to_string(),
                    span: ParserSpan::new(0, 0),
                    important: false,
                })
                .collect(),
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_all_margin_longhands() {
        let node = style_with_props(&[
            ("margin-top", "1px"),
            ("margin-right", "2px"),
            ("margin-bottom", "3px"),
            ("margin-left", "4px"),
        ]);
        let d = DeclarationBlockNoRedundantLonghandProperties.check(&node, &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("margin"));
    }

    #[test]
    fn allows_incomplete_longhands() {
        let node = style_with_props(&[("margin-top", "1px"), ("margin-bottom", "3px")]);
        assert!(DeclarationBlockNoRedundantLonghandProperties.check(&node, &ctx()).is_empty());
    }

    #[test]
    fn reports_overflow_longhands() {
        let node = style_with_props(&[("overflow-x", "hidden"), ("overflow-y", "auto")]);
        let d = DeclarationBlockNoRedundantLonghandProperties.check(&node, &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("overflow"));
    }
}
