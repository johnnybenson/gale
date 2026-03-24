use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Edit, Fix, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require a leading zero for fractional numbers less than 1.
///
/// Equivalent to Stylelint's `number-leading-zero` rule with "always" option.
pub struct NumberLeadingZero;

impl Rule for NumberLeadingZero {
    fn name(&self) -> &'static str {
        "number-leading-zero"
    }

    fn description(&self) -> &'static str {
        "Require a leading zero for fractional numbers less than 1"
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
            find_missing_leading_zeros(&decl.value, decl.span.offset, self, &mut diags);
        }
        diags
    }
}

fn find_missing_leading_zeros(
    value: &str,
    base_offset: usize,
    rule: &NumberLeadingZero,
    diags: &mut Vec<Diagnostic>,
) {
    let bytes = value.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'.' && (i + 1 < len) && bytes[i + 1].is_ascii_digit() {
            // Check if preceded by a digit — if so, this is not a missing leading zero
            let preceded_by_digit = i > 0 && bytes[i - 1].is_ascii_digit();
            if !preceded_by_digit {
                // Find the end of the number
                let start = i;
                let mut j = i + 1;
                while j < len && (bytes[j].is_ascii_digit() || bytes[j] == b'.') {
                    j += 1;
                }
                let number_text = &value[start..j];
                let replacement = format!("0{number_text}");
                diags.push(
                    Diagnostic::new(
                        rule.name(),
                        format!("Expected a leading zero before \"{number_text}\""),
                    )
                    .severity(rule.default_severity())
                    .span(Span::new(base_offset + start, j - start))
                    .fix(Fix::new(
                        "Add leading zero",
                        vec![Edit::new(
                            Span::new(base_offset + start, j - start),
                            &replacement,
                        )],
                    )),
                );
                i = j;
                continue;
            }
        }
        i += 1;
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

    fn style_with_value(val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "opacity".to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, val.len()),
                important: false,
            }],
            span: ParserSpan::new(0, 0),
            ..Default::default()
        })
    }

    #[test]
    fn reports_missing_leading_zero() {
        let d = NumberLeadingZero.check(&style_with_value(".5"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains(".5"));
        let fix = d[0].fix.as_ref().unwrap();
        assert_eq!(fix.edits[0].new_text, "0.5");
    }

    #[test]
    fn allows_leading_zero() {
        let d = NumberLeadingZero.check(&style_with_value("0.5"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn reports_in_complex_value() {
        let d = NumberLeadingZero.check(&style_with_value("rgba(0, 0, 0, .5)"), &ctx());
        assert_eq!(d.len(), 1);
        let fix = d[0].fix.as_ref().unwrap();
        assert_eq!(fix.edits[0].new_text, "0.5");
    }
}
