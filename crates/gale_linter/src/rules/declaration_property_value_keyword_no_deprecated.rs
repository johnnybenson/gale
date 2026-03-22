use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Check whether a property + value combination uses a deprecated keyword.
/// Returns `Some((deprecated_keyword, suggestion))` if deprecated.
fn check_deprecated_keyword(property: &str, value: &str) -> Option<(&'static str, &'static str)> {
    let prop = property.to_ascii_lowercase();
    let val = value.to_ascii_lowercase();

    // Trim the value to just the keyword (handle multi-token values by checking
    // if any token matches).
    let tokens: Vec<&str> = val.split_whitespace().collect();

    match prop.as_str() {
        "overflow" | "overflow-x" | "overflow-y" => {
            if tokens.iter().any(|t| *t == "overlay") {
                return Some(("overlay", "auto"));
            }
        }
        "text-justify" => {
            if tokens.iter().any(|t| *t == "distribute") {
                return Some(("distribute", "inter-character"));
            }
        }
        "word-break" => {
            if tokens.iter().any(|t| *t == "break-word") {
                return Some(("break-word", "overflow-wrap: anywhere"));
            }
        }
        _ => {}
    }

    None
}

pub struct DeclarationPropertyValueKeywordNoDeprecated;

impl Rule for DeclarationPropertyValueKeywordNoDeprecated {
    fn name(&self) -> &'static str {
        "declaration-property-value-keyword-no-deprecated"
    }

    fn description(&self) -> &'static str {
        "Disallow deprecated keyword values for properties"
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
            if let Some((keyword, suggestion)) =
                check_deprecated_keyword(&decl.property, &decl.value)
            {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Unexpected deprecated keyword \"{}\" for property \"{}\". Use \"{}\" instead.",
                            keyword, decl.property, suggestion
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

    fn style_node(sel: &str, props: &[(&str, &str)]) -> CssNode {
        CssNode::Style(StyleRule {
            selector: sel.to_string(),
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
    fn reports_overflow_overlay() {
        let node = style_node("a", &[("overflow", "overlay")]);
        let d = DeclarationPropertyValueKeywordNoDeprecated.check(&node, &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("overlay"));
        assert!(d[0].message.contains("auto"));
    }

    #[test]
    fn reports_text_justify_distribute() {
        let node = style_node("a", &[("text-justify", "distribute")]);
        let d = DeclarationPropertyValueKeywordNoDeprecated.check(&node, &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("distribute"));
        assert!(d[0].message.contains("inter-character"));
    }

    #[test]
    fn reports_word_break_break_word() {
        let node = style_node("a", &[("word-break", "break-word")]);
        let d = DeclarationPropertyValueKeywordNoDeprecated.check(&node, &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("break-word"));
        assert!(d[0].message.contains("overflow-wrap: anywhere"));
    }

    #[test]
    fn allows_overflow_auto() {
        let node = style_node("a", &[("overflow", "auto")]);
        assert!(
            DeclarationPropertyValueKeywordNoDeprecated
                .check(&node, &ctx())
                .is_empty()
        );
    }

    #[test]
    fn allows_normal_word_break() {
        let node = style_node("a", &[("word-break", "normal")]);
        assert!(
            DeclarationPropertyValueKeywordNoDeprecated
                .check(&node, &ctx())
                .is_empty()
        );
    }

    #[test]
    fn reports_overflow_x_overlay() {
        let node = style_node("a", &[("overflow-x", "overlay")]);
        let d = DeclarationPropertyValueKeywordNoDeprecated.check(&node, &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("overlay"));
    }
}
