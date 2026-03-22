use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require specific display notation (short or long form).
///
/// Equivalent to Stylelint's `declaration-property-value-disallowed-list` for display,
/// or a dedicated `display-notation` rule.
///
/// In "short" mode (default): flags multi-keyword display values when a short
/// equivalent exists. In "long" mode: flags short values when a long equivalent exists.
///
/// Mappings:
/// - `block flow` <-> `block`
/// - `inline flow` <-> `inline`
/// - `run-in flow` <-> `run-in`
/// - `block flex` <-> `flex`
/// - `block grid` <-> `grid`
/// - `inline flex` <-> `inline-flex`
/// - `inline grid` <-> `inline-grid`
/// - `block flow-root` <-> `flow-root`
/// - `inline flow-root` <-> `inline-block`
/// - `block table` <-> `table`
/// - `inline table` <-> `inline-table`
pub struct DisplayNotation;

/// (long form, short form) pairs.
const DISPLAY_MAPPINGS: &[(&str, &str)] = &[
    ("block flow", "block"),
    ("inline flow", "inline"),
    ("run-in flow", "run-in"),
    ("block flex", "flex"),
    ("block grid", "grid"),
    ("inline flex", "inline-flex"),
    ("inline grid", "inline-grid"),
    ("block flow-root", "flow-root"),
    ("inline flow-root", "inline-block"),
    ("block table", "table"),
    ("inline table", "inline-table"),
];

impl Rule for DisplayNotation {
    fn name(&self) -> &'static str {
        "display-notation"
    }

    fn description(&self) -> &'static str {
        "Specify short or long form for display values"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        let mode = ctx.options.and_then(|v| v.as_str()).unwrap_or("short");

        let mut diags = Vec::new();
        for decl in &rule.declarations {
            if decl.property.to_ascii_lowercase() != "display" {
                continue;
            }

            let value = decl.value.trim().to_ascii_lowercase();

            match mode {
                "short" => {
                    // Flag long forms that have a short equivalent
                    for &(long, short) in DISPLAY_MAPPINGS {
                        if value == long {
                            diags.push(
                                Diagnostic::new(
                                    self.name(),
                                    format!("Expected \"{short}\" instead of \"{long}\""),
                                )
                                .severity(self.default_severity())
                                .span(Span::new(decl.span.offset, decl.span.length)),
                            );
                            break;
                        }
                    }
                }
                "long" => {
                    // Flag short forms that have a long equivalent
                    for &(long, short) in DISPLAY_MAPPINGS {
                        if value == short {
                            diags.push(
                                Diagnostic::new(
                                    self.name(),
                                    format!("Expected \"{long}\" instead of \"{short}\""),
                                )
                                .severity(self.default_severity())
                                .span(Span::new(decl.span.offset, decl.span.length)),
                            );
                            break;
                        }
                    }
                }
                _ => {}
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

    fn ctx_with_options(options: serde_json::Value) -> RuleContext<'static> {
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: Some(Box::leak(Box::new(options))),
        }
    }

    fn style_with_decl(prop: &str, val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: prop.to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, prop.len() + val.len() + 2),
                important: false,
            }],
span: ParserSpan::new(0, 0),
            ..Default::default()
})
    }

    #[test]
    fn short_mode_flags_long_form() {
        // Default mode is "short"
        let d = DisplayNotation.check(&style_with_decl("display", "block flow"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("\"block\""));
    }

    #[test]
    fn short_mode_allows_short_form() {
        let d = DisplayNotation.check(&style_with_decl("display", "block"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn long_mode_flags_short_form() {
        let ctx = ctx_with_options(serde_json::json!("long"));
        let d = DisplayNotation.check(&style_with_decl("display", "flex"), &ctx);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("\"block flex\""));
    }

    #[test]
    fn long_mode_allows_long_form() {
        let ctx = ctx_with_options(serde_json::json!("long"));
        let d = DisplayNotation.check(&style_with_decl("display", "block flex"), &ctx);
        assert!(d.is_empty());
    }

    #[test]
    fn ignores_non_display_properties() {
        let d = DisplayNotation.check(&style_with_decl("color", "block flow"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn inline_block_mapping() {
        let ctx = ctx_with_options(serde_json::json!("long"));
        let d = DisplayNotation.check(&style_with_decl("display", "inline-block"), &ctx);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("\"inline flow-root\""));
    }

    #[test]
    fn rule_name_is_correct() {
        assert_eq!(DisplayNotation.name(), "display-notation");
    }
}
