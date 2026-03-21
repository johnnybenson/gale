use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Prefer numeric font-weight notation (`bold` → `700`, `normal` → `400`).
///
/// Equivalent to Stylelint's `font-weight-notation` rule with "numeric" option.
/// Checks both `font-weight` and `font` shorthand declarations. Detection-only.
pub struct FontWeightNotation;

/// Named font-weight keywords that have numeric equivalents.
const NAMED_WEIGHTS: &[&str] = &["bold", "normal"];

impl Rule for FontWeightNotation {
    fn name(&self) -> &'static str {
        "font-weight-notation"
    }

    fn description(&self) -> &'static str {
        "Require numeric font-weight values"
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
            let value_lower = decl.value.to_ascii_lowercase();

            if prop_lower == "font-weight" {
                for &kw in NAMED_WEIGHTS {
                    if value_lower.trim() == kw {
                        diags.push(
                            Diagnostic::new(
                                self.name(),
                                format!(
                                    "Expected numeric font-weight notation instead of \"{kw}\""
                                ),
                            )
                            .severity(self.default_severity())
                            .span(Span::new(decl.span.offset, decl.span.length)),
                        );
                    }
                }
            } else if prop_lower == "font" {
                // In the font shorthand, check for named weight keywords as whole words.
                let tokens: Vec<&str> = value_lower.split_whitespace().collect();
                for &kw in NAMED_WEIGHTS {
                    if tokens.contains(&kw) {
                        diags.push(
                            Diagnostic::new(
                                self.name(),
                                format!(
                                    "Expected numeric font-weight notation instead of \"{kw}\" in font shorthand"
                                ),
                            )
                            .severity(self.default_severity())
                            .span(Span::new(decl.span.offset, decl.span.length)),
                        );
                    }
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
        }
    }

    fn style_with_decl(prop: &str, value: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: prop.to_string(),
                value: value.to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_bold_keyword() {
        let d = FontWeightNotation.check(&style_with_decl("font-weight", "bold"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("bold"));
    }

    #[test]
    fn reports_normal_keyword() {
        let d = FontWeightNotation.check(&style_with_decl("font-weight", "normal"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("normal"));
    }

    #[test]
    fn allows_numeric_weight() {
        let d = FontWeightNotation.check(&style_with_decl("font-weight", "700"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn reports_bold_in_font_shorthand() {
        let d = FontWeightNotation.check(&style_with_decl("font", "bold 16px Arial"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("bold"));
    }

    #[test]
    fn allows_numeric_in_font_shorthand() {
        let d = FontWeightNotation.check(&style_with_decl("font", "700 16px Arial"), &ctx());
        assert!(d.is_empty());
    }
}
