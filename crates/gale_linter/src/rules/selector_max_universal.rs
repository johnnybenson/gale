use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Limit the number of universal selectors (`*`) in a selector.
///
/// Equivalent to Stylelint's `selector-max-universal` rule.
/// Default maximum: 1.
pub struct SelectorMaxUniversal;

const DEFAULT_MAX: usize = 1;

impl Rule for SelectorMaxUniversal {
    fn name(&self) -> &'static str {
        "selector-max-universal"
    }

    fn description(&self) -> &'static str {
        "Limit the number of universal selectors in a selector"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        let max = ctx
            .options
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_MAX);

        let mut diags = Vec::new();

        for sel in rule.selector.split(',') {
            let sel = sel.trim();
            if sel.is_empty() {
                continue;
            }
            let count = count_universal_selectors(sel);
            if count > max {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected selector \"{sel}\" to have no more than {max} universal selector(s), found {count}",
                        ),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(rule.span.offset, rule.span.length)),
                );
            }
        }

        diags
    }
}

/// Count universal selectors (`*`) in a selector string.
/// Skips `*` inside quoted strings and attribute selectors `[...]`.
fn count_universal_selectors(selector: &str) -> usize {
    let mut count = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_attribute = 0; // depth of [ ]
    let chars: Vec<char> = selector.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];
        match ch {
            '\\' => {
                i += 2; // skip escaped char
                continue;
            }
            '\'' if !in_double_quote && in_attribute == 0 => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote && in_attribute == 0 => {
                in_double_quote = !in_double_quote;
            }
            '[' if !in_single_quote && !in_double_quote => {
                in_attribute += 1;
            }
            ']' if !in_single_quote && !in_double_quote && in_attribute > 0 => {
                in_attribute -= 1;
            }
            '*' if !in_single_quote && !in_double_quote && in_attribute == 0 => {
                count += 1;
            }
            _ => {}
        }
        i += 1;
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
            options: None,
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
    fn reports_too_many_universal() {
        let d = SelectorMaxUniversal.check(
            &style_with_selector("* *"),
            &ctx(),
        );
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("found 2"));
    }

    #[test]
    fn allows_single_universal() {
        let d = SelectorMaxUniversal.check(
            &style_with_selector("*"),
            &ctx(),
        );
        assert!(d.is_empty());
    }

    #[test]
    fn allows_no_universal() {
        let d = SelectorMaxUniversal.check(
            &style_with_selector("div .class"),
            &ctx(),
        );
        assert!(d.is_empty());
    }

    #[test]
    fn skips_star_in_attribute_value() {
        // [attr*=val] contains * but it's not a universal selector
        let d = SelectorMaxUniversal.check(
            &style_with_selector("div[class*='foo']"),
            &ctx(),
        );
        assert!(d.is_empty());
    }

    #[test]
    fn checks_each_selector_separately() {
        // Each individual selector has 1 universal, within the default max
        let d = SelectorMaxUniversal.check(
            &style_with_selector("*, *"),
            &ctx(),
        );
        assert!(d.is_empty());
    }

    #[test]
    fn respects_custom_max() {
        let options = serde_json::json!(2);
        let ctx = RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: Some(&options),
        };
        let d = SelectorMaxUniversal.check(
            &style_with_selector("* *"),
            &ctx,
        );
        assert!(d.is_empty());
    }
}
