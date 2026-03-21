use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Limit the number of class selectors in a selector.
///
/// Equivalent to Stylelint's `selector-max-class` rule.
/// Default maximum: 3. Detection-only.
pub struct SelectorMaxClass;

const MAX_CLASS: usize = 3;

impl Rule for SelectorMaxClass {
    fn name(&self) -> &'static str {
        "selector-max-class"
    }

    fn description(&self) -> &'static str {
        "Limit the number of class selectors in a selector"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let count = count_class_selectors(&rule.selector);
        if count > MAX_CLASS {
            vec![Diagnostic::new(
                self.name(),
                format!(
                    "Expected selector \"{}\" to have no more than {MAX_CLASS} class selector(s), found {count}",
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

/// Count `.` characters that are class selectors (followed by a CSS ident start character).
fn count_class_selectors(selector: &str) -> usize {
    let chars: Vec<char> = selector.chars().collect();
    let mut count = 0;
    for (i, &ch) in chars.iter().enumerate() {
        if ch == '.'
            && let Some(&next) = chars.get(i + 1)
            && (next.is_ascii_alphabetic() || next == '_' || next == '-' || !next.is_ascii())
        {
            count += 1;
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
    fn reports_too_many_classes() {
        let d = SelectorMaxClass.check(&style_with_selector(".a .b .c .d"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("found 4"));
    }

    #[test]
    fn allows_within_limit() {
        let d = SelectorMaxClass.check(&style_with_selector(".a .b .c"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_no_classes() {
        let d = SelectorMaxClass.check(&style_with_selector("div span"), &ctx());
        assert!(d.is_empty());
    }
}
