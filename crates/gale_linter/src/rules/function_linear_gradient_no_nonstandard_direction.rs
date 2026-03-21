use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow non-standard direction values in `linear-gradient()` calls.
///
/// The standard syntax uses `to top`, `to right`, etc. Non-standard
/// (legacy WebKit) syntax uses `top`, `left`, etc. without the `to` keyword.
///
/// Equivalent to Stylelint's `function-linear-gradient-no-nonstandard-direction` rule.
pub struct FunctionLinearGradientNoNonstandardDirection;

/// Direction keywords that are non-standard when used without `to`.
const NON_STANDARD_DIRECTIONS: &[&str] = &[
    "top",
    "bottom",
    "left",
    "right",
    "top left",
    "top right",
    "bottom left",
    "bottom right",
    "left top",
    "left bottom",
    "right top",
    "right bottom",
];

/// Function names that use standard gradient direction syntax.
const GRADIENT_FUNCTIONS: &[&str] = &[
    "linear-gradient",
    "repeating-linear-gradient",
    "-webkit-linear-gradient",
    "-moz-linear-gradient",
    "-o-linear-gradient",
];

impl Rule for FunctionLinearGradientNoNonstandardDirection {
    fn name(&self) -> &'static str {
        "function-linear-gradient-no-nonstandard-direction"
    }

    fn description(&self) -> &'static str {
        "Disallow direction values in linear-gradient() calls that are not valid according to the standard syntax"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let declarations: Vec<&gale_css_parser::Declaration> = match node {
            CssNode::Style(rule) => rule.declarations.iter().collect(),
            CssNode::Declaration(decl) => vec![decl],
            _ => return vec![],
        };
        for decl in declarations {
            check_value(&decl.value, decl.span.offset, self, &mut diags);
        }
        diags
    }
}

fn check_value(
    value: &str,
    base_offset: usize,
    rule: &FunctionLinearGradientNoNonstandardDirection,
    diags: &mut Vec<Diagnostic>,
) {
    let lower = value.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'(' && i > 0 {
            // Find start of function name.
            let paren_pos = i;
            let mut start = i;
            while start > 0
                && (bytes[start - 1].is_ascii_alphanumeric()
                    || bytes[start - 1] == b'-'
                    || bytes[start - 1] == b'_')
            {
                start -= 1;
            }
            let fname = &lower[start..paren_pos];
            if GRADIENT_FUNCTIONS.contains(&fname) {
                // Extract the first argument (before the first comma).
                let after_paren = paren_pos + 1;
                // Find matching close paren.
                let mut depth = 1i32;
                let mut close = after_paren;
                for (j, &byte) in bytes.iter().enumerate().take(len).skip(after_paren) {
                    if byte == b'(' {
                        depth += 1;
                    } else if byte == b')' {
                        depth -= 1;
                        if depth == 0 {
                            close = j;
                            break;
                        }
                    }
                }
                // Get first argument (before first comma at depth 0).
                let args_area = &lower[after_paren..close];
                let first_arg = if let Some(comma) = find_top_level_comma(args_area) {
                    args_area[..comma].trim()
                } else {
                    args_area.trim()
                };

                // Check if the first argument is a non-standard direction.
                if NON_STANDARD_DIRECTIONS.contains(&first_arg) {
                    diags.push(
                        Diagnostic::new(
                            rule.name(),
                            format!(
                                "Unexpected non-standard direction \"{}\" in {fname}()",
                                first_arg
                            ),
                        )
                        .severity(rule.default_severity())
                        .span(Span::new(base_offset + start, close + 1 - start)),
                    );
                }

                i = close + 1;
                continue;
            }
        }
        i += 1;
    }
}

/// Find the position of the first comma at depth 0 (not inside nested parens).
fn find_top_level_comma(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, b) in s.bytes().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
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
                property: "background".to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, val.len()),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    fn decl_with_value(val: &str) -> CssNode {
        CssNode::Declaration(Declaration {
            property: "background".to_string(),
            value: val.to_string(),
            span: ParserSpan::new(0, val.len()),
            important: false,
        })
    }

    #[test]
    fn reports_nonstandard_direction() {
        let d = FunctionLinearGradientNoNonstandardDirection
            .check(&style_with_value("linear-gradient(top, red, blue)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("top"));
    }

    #[test]
    fn accepts_standard_direction() {
        let d = FunctionLinearGradientNoNonstandardDirection.check(
            &style_with_value("linear-gradient(to top, red, blue)"),
            &ctx(),
        );
        assert!(d.is_empty());
    }

    #[test]
    fn accepts_angle() {
        let d = FunctionLinearGradientNoNonstandardDirection.check(
            &style_with_value("linear-gradient(45deg, red, blue)"),
            &ctx(),
        );
        assert!(d.is_empty());
    }

    #[test]
    fn reports_nonstandard_in_declaration_node() {
        let d = FunctionLinearGradientNoNonstandardDirection.check(
            &decl_with_value("linear-gradient(bottom, red, blue)"),
            &ctx(),
        );
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn reports_two_word_direction() {
        let d = FunctionLinearGradientNoNonstandardDirection.check(
            &style_with_value("linear-gradient(top right, red, blue)"),
            &ctx(),
        );
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("top right"));
    }

    #[test]
    fn ignores_non_gradient() {
        let d = FunctionLinearGradientNoNonstandardDirection
            .check(&style_with_value("rgb(255, 0, 0)"), &ctx());
        assert!(d.is_empty());
    }
}
