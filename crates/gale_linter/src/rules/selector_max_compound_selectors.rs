use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Limit the number of compound selectors in a selector.
///
/// Equivalent to Stylelint's `selector-max-compound-selectors` rule.
/// Default maximum: 3. Counts parts separated by combinators (space, `>`, `+`, `~`).
pub struct SelectorMaxCompoundSelectors;

const MAX_COMPOUND: usize = 3;

impl Rule for SelectorMaxCompoundSelectors {
    fn name(&self) -> &'static str {
        "selector-max-compound-selectors"
    }

    fn description(&self) -> &'static str {
        "Limit the number of compound selectors in a selector"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        let selector = if matches!(ctx.syntax, gale_css_parser::Syntax::Scss | gale_css_parser::Syntax::Sass | gale_css_parser::Syntax::Less) {
            strip_scss_line_comments(&rule.selector)
        } else {
            rule.selector.clone()
        };

        let mut diags = Vec::new();
        // Check each comma-separated selector
        for sel in selector.split(',') {
            let sel = sel.trim();
            let count = count_compound_selectors(sel);
            if count > MAX_COMPOUND {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected selector \"{sel}\" to have no more than {MAX_COMPOUND} compound selector(s), found {count}"
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

/// Count compound selectors by splitting on combinators (whitespace, `>`, `+`, `~`).
fn count_compound_selectors(selector: &str) -> usize {
    if selector.is_empty() {
        return 0;
    }

    let mut count = 1usize;
    let chars: Vec<char> = selector.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_brackets = 0i32;

    while i < len {
        // Skip attribute selectors and pseudo-function arguments
        if chars[i] == '[' || chars[i] == '(' {
            in_brackets += 1;
            i += 1;
            continue;
        }
        if chars[i] == ']' || chars[i] == ')' {
            in_brackets -= 1;
            i += 1;
            continue;
        }

        if in_brackets > 0 {
            i += 1;
            continue;
        }

        // Explicit combinators
        if chars[i] == '>' || chars[i] == '+' || chars[i] == '~' {
            count += 1;
            i += 1;
            // Skip following whitespace
            while i < len && chars[i].is_ascii_whitespace() {
                i += 1;
            }
            continue;
        }

        // Descendant combinator (whitespace between simple selectors)
        if chars[i].is_ascii_whitespace() {
            // Skip all whitespace
            while i < len && chars[i].is_ascii_whitespace() {
                i += 1;
            }
            // If not at end and next char is not a combinator, it's a descendant combinator
            if i < len && chars[i] != '>' && chars[i] != '+' && chars[i] != '~' {
                count += 1;
            }
            continue;
        }

        i += 1;
    }

    count
}

/// Strip `//` line comments from a selector string (SCSS/Less).
fn strip_scss_line_comments(selector: &str) -> String {
    selector
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
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
    fn reports_too_many_compound_selectors() {
        // 4 compound selectors: .a .b .c .d
        let d = SelectorMaxCompoundSelectors
            .check(&style_with_selector(".a .b .c .d"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("found 4"));
    }

    #[test]
    fn allows_within_limit() {
        // 3 compound selectors: .a .b .c
        let d = SelectorMaxCompoundSelectors
            .check(&style_with_selector(".a .b .c"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn counts_child_combinator() {
        // 4 compound selectors: .a > .b > .c > .d
        let d = SelectorMaxCompoundSelectors
            .check(&style_with_selector(".a > .b > .c > .d"), &ctx());
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn single_selector_ok() {
        let d = SelectorMaxCompoundSelectors
            .check(&style_with_selector(".foo"), &ctx());
        assert!(d.is_empty());
    }
}
