use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow named colors in CSS declarations.
///
/// Equivalent to Stylelint's `color-named` rule with "never" option.
pub struct ColorNamed;

const NAMED_COLORS: &[&str] = &[
    "red", "blue", "green", "yellow", "orange", "purple", "pink", "black", "white", "gray", "grey",
    "cyan", "magenta", "lime", "olive", "navy", "teal", "aqua", "fuchsia", "maroon", "silver",
];

impl Rule for ColorNamed {
    fn name(&self) -> &'static str {
        "color-named"
    }

    fn description(&self) -> &'static str {
        "Disallow named colors"
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
            let value_lower = decl.value.to_ascii_lowercase();
            for &color in NAMED_COLORS {
                if contains_word(&value_lower, color) {
                    diags.push(
                        Diagnostic::new(
                            self.name(),
                            format!(
                                "Unexpected named color \"{color}\" in declaration \"{}\"",
                                decl.property
                            ),
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

/// Check if `haystack` contains `word` as a whole word (bounded by non-alphanumeric chars or string edges).
fn contains_word(haystack: &str, word: &str) -> bool {
    let bytes = haystack.as_bytes();
    let word_bytes = word.as_bytes();
    let wlen = word_bytes.len();
    if bytes.len() < wlen {
        return false;
    }
    for i in 0..=(bytes.len() - wlen) {
        if &bytes[i..i + wlen] == word_bytes {
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            let after_ok = i + wlen == bytes.len() || !bytes[i + wlen].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return true;
            }
        }
    }
    false
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
    fn reports_named_color() {
        let d = ColorNamed.check(&style_with_value("red"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("red"));
    }

    #[test]
    fn allows_hex_color() {
        let d = ColorNamed.check(&style_with_value("#ff0000"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn does_not_match_partial_words() {
        // "grayed" should not match "gray"
        let d = ColorNamed.check(&style_with_value("grayed"), &ctx());
        assert!(d.is_empty());
    }
}
