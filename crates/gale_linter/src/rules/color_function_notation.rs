use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Prefer modern color function notation (space-separated) over legacy (comma-separated).
///
/// Equivalent to Stylelint's `color-function-notation` rule with "modern" option.
/// Detects comma-separated arguments in rgb/rgba/hsl/hsla. Detection-only.
pub struct ColorFunctionNotation;

const COLOR_FUNCTIONS: &[&str] = &["rgb(", "rgba(", "hsl(", "hsla("];

impl Rule for ColorFunctionNotation {
    fn name(&self) -> &'static str {
        "color-function-notation"
    }

    fn description(&self) -> &'static str {
        "Specify modern or legacy notation for color functions"
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
            let lower = decl.value.to_ascii_lowercase();
            for &func in COLOR_FUNCTIONS {
                let mut search_from = 0;
                while let Some(pos) = lower[search_from..].find(func) {
                    let abs_pos = search_from + pos;
                    let args_start = abs_pos + func.len();
                    if let Some(close) = lower[args_start..].find(')') {
                        let args = &decl.value[args_start..args_start + close];
                        if args.contains(',') {
                            let fn_name = &func[..func.len() - 1]; // strip trailing (
                            diags.push(
                                Diagnostic::new(
                                    self.name(),
                                    format!(
                                        "Expected modern color function notation for {fn_name}()"
                                    ),
                                )
                                .severity(self.default_severity())
                                .span(Span::new(decl.span.offset, decl.span.length)),
                            );
                        }
                    }
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
            syntax: Syntax::Css, options: None }
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
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_legacy_rgb() {
        let d = ColorFunctionNotation.check(&style_with_value("rgb(0, 0, 0)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("rgb()"));
    }

    #[test]
    fn allows_modern_rgb() {
        let d = ColorFunctionNotation.check(&style_with_value("rgb(0 0 0)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn reports_legacy_hsl() {
        let d = ColorFunctionNotation.check(&style_with_value("hsl(0, 100%, 50%)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("hsl()"));
    }

    #[test]
    fn allows_modern_hsl() {
        let d = ColorFunctionNotation.check(&style_with_value("hsl(0 100% 50%)"), &ctx());
        assert!(d.is_empty());
    }
}
