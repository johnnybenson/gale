use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Prefer degree notation for hue values in `hsl()`/`hsla()`/`hwb()`.
///
/// Equivalent to Stylelint's `hue-degree-notation` rule with "angle" option.
/// Detects bare numbers used as hue values without the `deg` unit. Detection-only.
pub struct HueDegreeNotation;

const HUE_FUNCTIONS: &[&str] = &["hsl(", "hsla(", "hwb("];

impl Rule for HueDegreeNotation {
    fn name(&self) -> &'static str {
        "hue-degree-notation"
    }

    fn description(&self) -> &'static str {
        "Specify number or angle notation for degree hues"
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
            for &func in HUE_FUNCTIONS {
                let mut search_from = 0;
                while let Some(pos) = lower[search_from..].find(func) {
                    let abs_pos = search_from + pos;
                    let args_start = abs_pos + func.len();
                    if let Some(close) = lower[args_start..].find(')') {
                        let args = &lower[args_start..args_start + close];
                        // The first argument is the hue. Split by space or comma.
                        let hue = if args.contains(',') {
                            args.split(',').next().map(|s| s.trim())
                        } else {
                            args.split_whitespace().next()
                        };
                        if let Some(hue_val) = hue {
                            // If hue is a bare number (no unit like deg/rad/grad/turn), report.
                            if is_bare_number(hue_val) {
                                let fn_name = &func[..func.len() - 1];
                                diags.push(
                                    Diagnostic::new(
                                        self.name(),
                                        format!("Expected degree notation for hue in {fn_name}()"),
                                    )
                                    .severity(self.default_severity())
                                    .span(Span::new(decl.span.offset, decl.span.length)),
                                );
                            }
                        }
                    }
                    search_from = abs_pos + 1;
                }
            }
        }
        diags
    }
}

/// Check if a value is a bare number (no angle unit suffix).
fn is_bare_number(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    // If it ends with an angle unit, it's not bare.
    for unit in &["deg", "rad", "grad", "turn"] {
        if s.ends_with(unit) {
            return false;
        }
    }
    // Must parse as a number.
    let s = s.trim();
    let mut has_digit = false;
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_digit() {
            has_digit = true;
        } else if c == '.' || ((c == '-' || c == '+') && i == 0) {
            // ok
        } else {
            return false;
        }
    }
    has_digit
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
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_bare_number_hue_in_hsl() {
        let d = HueDegreeNotation.check(&style_with_value("hsl(0 100% 50%)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("hsl()"));
    }

    #[test]
    fn allows_degree_notation() {
        let d = HueDegreeNotation.check(&style_with_value("hsl(0deg 100% 50%)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn reports_bare_number_in_hwb() {
        let d = HueDegreeNotation.check(&style_with_value("hwb(120 0% 0%)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("hwb()"));
    }

    #[test]
    fn allows_rad_unit() {
        let d = HueDegreeNotation.check(&style_with_value("hsl(3.14rad 100% 50%)"), &ctx());
        assert!(d.is_empty());
    }
}
