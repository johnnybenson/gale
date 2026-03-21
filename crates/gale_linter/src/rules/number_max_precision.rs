use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports numbers with more than 4 decimal places.
///
/// Equivalent to Stylelint's `number-max-precision` rule (default: 4).
pub struct NumberMaxPrecision;

const MAX_PRECISION: usize = 4;

impl Rule for NumberMaxPrecision {
    fn name(&self) -> &'static str {
        "number-max-precision"
    }

    fn description(&self) -> &'static str {
        "Limit the number of decimal places allowed in numbers"
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
            if exceeds_precision(&decl.value) {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected number to have no more than {MAX_PRECISION} decimal places in \"{}\"",
                            decl.value,
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

fn exceeds_precision(value: &str) -> bool {
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '.' {
            // Count digits after the decimal point
            let mut decimal_digits = 0;
            let mut j = i + 1;
            while j < len && chars[j].is_ascii_digit() {
                decimal_digits += 1;
                j += 1;
            }
            if decimal_digits > MAX_PRECISION {
                return true;
            }
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css }
    }

    fn style_decl(val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "width".to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_excess_precision() {
        let d = NumberMaxPrecision.check(&style_decl("0.12345em"), &ctx());
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn allows_within_precision() {
        assert!(NumberMaxPrecision.check(&style_decl("0.1234em"), &ctx()).is_empty());
        assert!(NumberMaxPrecision.check(&style_decl("10px"), &ctx()).is_empty());
    }

    #[test]
    fn reports_excess_in_multiple_values() {
        let d = NumberMaxPrecision.check(&style_decl("0.12345 0.6789"), &ctx());
        assert_eq!(d.len(), 1);
    }
}
