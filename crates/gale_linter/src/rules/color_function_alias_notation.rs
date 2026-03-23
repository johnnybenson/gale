use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require modern or legacy color function notation.
///
/// In modern CSS, `rgb()` accepts an optional alpha channel as the 4th argument,
/// making `rgba()` redundant. The same applies to `hsl()` vs `hsla()`.
///
/// Equivalent to Stylelint's `color-function-alias-notation` rule with "modern" option.
/// In "modern" mode, flags usage of `rgba()` (prefer `rgb()`) and `hsla()` (prefer `hsl()`).
pub struct ColorFunctionAliasNotation;

const ALIAS_FUNCTIONS: &[(&str, &str)] = &[("rgba(", "rgb"), ("hsla(", "hsl")];

impl Rule for ColorFunctionAliasNotation {
    fn name(&self) -> &'static str {
        "color-function-alias-notation"
    }

    fn description(&self) -> &'static str {
        "Specify modern or legacy notation for color function aliases"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let decls: Vec<&gale_css_parser::Declaration> = match node {
            CssNode::Style(rule) => rule.declarations.iter().collect(),
            CssNode::Declaration(decl) => vec![decl],
            _ => return vec![],
        };
        let mut diags = Vec::new();
        for decl in decls {
            let lower = decl.value.to_ascii_lowercase();
            for &(alias, modern) in ALIAS_FUNCTIONS {
                let mut search_from = 0;
                while let Some(pos) = lower[search_from..].find(alias) {
                    let abs_pos = search_from + pos;
                    let legacy = &alias[..alias.len() - 1]; // "rgba" or "hsla"
                    diags.push(
                        Diagnostic::new(
                            self.name(),
                            format!(
                                "Expected \"{legacy}\" to be \"{modern}\""
                            ),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(decl.span.offset, decl.span.length)),
                    );
                    search_from = abs_pos + 1;
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

    fn style_with_value(value: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "color".to_string(),
                value: value.to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
span: ParserSpan::new(0, 0),
            ..Default::default()
})
    }

    #[test]
    fn reports_rgba() {
        let d = ColorFunctionAliasNotation.check(&style_with_value("rgba(0, 0, 0, 0.5)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("\"rgba\" to be \"rgb\""));
    }

    #[test]
    fn reports_hsla() {
        let d =
            ColorFunctionAliasNotation.check(&style_with_value("hsla(0, 100%, 50%, 0.8)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("\"hsla\" to be \"hsl\""));
    }

    #[test]
    fn allows_rgb() {
        let d = ColorFunctionAliasNotation.check(&style_with_value("rgb(0 0 0)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_hsl() {
        let d = ColorFunctionAliasNotation.check(&style_with_value("hsl(0 100% 50%)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_rgb_with_alpha() {
        let d = ColorFunctionAliasNotation.check(&style_with_value("rgb(0 0 0 / 0.5)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn reports_multiple_aliases() {
        let d = ColorFunctionAliasNotation.check(
            &style_with_value("rgba(0, 0, 0, 1), hsla(0, 0%, 0%, 1)"),
            &ctx(),
        );
        assert_eq!(d.len(), 2);
    }
}
