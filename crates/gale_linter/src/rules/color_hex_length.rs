use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports hex colors that can be shortened (e.g. #ffffff → #fff, #ffffffaa → #fffa).
///
/// Equivalent to Stylelint's `color-hex-length` rule.
pub struct ColorHexLength;

impl Rule for ColorHexLength {
    fn name(&self) -> &'static str {
        "color-hex-length"
    }

    fn description(&self) -> &'static str {
        "Disallow hex colors that can be shortened"
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
            for hex in find_hex_colors(&decl.value) {
                if can_shorten(&hex) {
                    diags.push(
                        Diagnostic::new(
                            self.name(),
                            format!("Expected \"{hex}\" to be \"{}\"", shorten(&hex)),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(decl.span.offset, decl.span.length)),
                    );
                }
            }
        }
        diags
    }
}

fn find_hex_colors(value: &str) -> Vec<String> {
    let mut colors = Vec::new();
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if chars[i] == '#' {
            let start = i;
            i += 1;
            while i < len && chars[i].is_ascii_hexdigit() {
                i += 1;
            }
            if i > start + 1 {
                let hex: String = chars[start..i].iter().collect();
                colors.push(hex);
            }
        } else {
            i += 1;
        }
    }
    colors
}

/// Check if a 6-digit hex can be shortened to 3, or 8-digit to 4.
fn can_shorten(hex: &str) -> bool {
    let digits: Vec<char> = hex[1..].chars().map(|c| c.to_ascii_lowercase()).collect();
    match digits.len() {
        6 => digits[0] == digits[1] && digits[2] == digits[3] && digits[4] == digits[5],
        8 => {
            digits[0] == digits[1]
                && digits[2] == digits[3]
                && digits[4] == digits[5]
                && digits[6] == digits[7]
        }
        _ => false,
    }
}

fn shorten(hex: &str) -> String {
    let digits: Vec<char> = hex[1..].chars().map(|c| c.to_ascii_lowercase()).collect();
    match digits.len() {
        6 => format!("#{}{}{}", digits[0], digits[2], digits[4]),
        8 => format!("#{}{}{}{}", digits[0], digits[2], digits[4], digits[6]),
        _ => hex.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css }
    }

    fn style_with_value(val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "color".to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_shortenable_6_digit_hex() {
        let d = ColorHexLength.check(&style_with_value("#ffffff"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("#fff"));
    }

    #[test]
    fn reports_shortenable_8_digit_hex() {
        // #ff00ffaa → all pairs match (ff, 00, ff, aa) → can shorten to #f0fa
        let d = ColorHexLength.check(&style_with_value("#ff00ffaa"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("#f0fa"));
        // #aabbccdd → can shorten to #abcd
        let d = ColorHexLength.check(&style_with_value("#aabbccdd"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("#abcd"));
    }

    #[test]
    fn allows_non_shortenable_8_digit_hex() {
        // #f100ffaa — f1 pair doesn't match
        assert!(ColorHexLength.check(&style_with_value("#f100ffaa"), &ctx()).is_empty());
        // #ffff01ff — 01 pair doesn't match
        assert!(ColorHexLength.check(&style_with_value("#ffff01ff"), &ctx()).is_empty());
    }

    #[test]
    fn allows_non_shortenable_hex() {
        assert!(ColorHexLength.check(&style_with_value("#f0f0f0"), &ctx()).is_empty());
        assert!(ColorHexLength.check(&style_with_value("#fff"), &ctx()).is_empty());
    }
}
