use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Limit the number of ID selectors in a selector.
///
/// Equivalent to Stylelint's `selector-max-id` rule.
/// Default maximum: 0 (disallow ID selectors entirely).
pub struct SelectorMaxId;

const MAX_ID: usize = 0;

impl Rule for SelectorMaxId {
    fn name(&self) -> &'static str {
        "selector-max-id"
    }

    fn description(&self) -> &'static str {
        "Limit the number of ID selectors in a selector"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let count = count_id_selectors(&rule.selector);
        if count > MAX_ID {
            vec![Diagnostic::new(
                self.name(),
                format!(
                    "Expected selector \"{}\" to have no more than {MAX_ID} ID selector(s), found {count}",
                    rule.selector
                ),
            )
            .severity(self.default_severity())
            .span(Span::new(rule.span.offset, rule.span.length))]
        } else {
            vec![]
        }
    }
}

/// Count `#` characters that are ID selectors (not hex color fragments).
/// A `#` is an ID selector if it is preceded by nothing, whitespace, or a combinator/selector char,
/// and followed by a CSS identifier start character (letter, underscore, hyphen, or non-ASCII).
fn count_id_selectors(selector: &str) -> usize {
    let chars: Vec<char> = selector.chars().collect();
    let mut count = 0;
    for (i, &ch) in chars.iter().enumerate() {
        if ch == '#' {
            // Check that the next char is a valid CSS ident start
            if let Some(&next) = chars.get(i + 1)
                && (next.is_ascii_alphabetic() || next == '_' || next == '-' || !next.is_ascii())
            {
                count += 1;
            }
        }
    }
    count
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

    fn style_with_selector(sel: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: sel.to_string(),
            declarations: vec![Declaration {
                property: "color".to_string(),
                value: "red".to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_id_selector() {
        let d = SelectorMaxId.check(&style_with_selector("#foo"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("#foo"));
    }

    #[test]
    fn allows_class_selector() {
        let d = SelectorMaxId.check(&style_with_selector(".bar"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn counts_multiple_ids() {
        let d = SelectorMaxId.check(&style_with_selector("#a #b"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("found 2"));
    }
}
