use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Prefer `:not()` pseudo-class with list arguments over chained `:not()`.
///
/// Equivalent to Stylelint's `selector-not-notation` rule with "complex" option.
/// Detects chained `:not()` like `:not(.a):not(.b)` which should be `:not(.a, .b)`.
/// Detection-only.
pub struct SelectorNotNotation;

impl Rule for SelectorNotNotation {
    fn name(&self) -> &'static str {
        "selector-not-notation"
    }

    fn description(&self) -> &'static str {
        "Specify simple or complex notation for :not() pseudo-class"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let lower = rule.selector.to_ascii_lowercase();
        if has_chained_not(&lower) {
            vec![Diagnostic::new(
                self.name(),
                format!(
                    "Expected :not() pseudo-class with list argument instead of chained :not() in \"{}\"",
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

/// Check if the selector contains chained `:not(...)` pseudo-classes, e.g. `:not(.a):not(.b)`.
fn has_chained_not(selector: &str) -> bool {
    let pattern = ":not(";
    let mut search_from = 0;
    let mut last_end: Option<usize> = None;

    while let Some(pos) = selector[search_from..].find(pattern) {
        let abs_pos = search_from + pos;
        let args_start = abs_pos + pattern.len();

        // Find the matching closing paren (handle nested parens).
        let mut depth = 1;
        let mut i = args_start;
        let bytes = selector.as_bytes();
        while i < bytes.len() && depth > 0 {
            if bytes[i] == b'(' {
                depth += 1;
            } else if bytes[i] == b')' {
                depth -= 1;
            }
            i += 1;
        }

        if depth == 0 {
            // `i` is one past the closing paren.
            if let Some(prev_end) = last_end
                && abs_pos == prev_end
            {
                return true;
            }
            last_end = Some(i);
        }

        search_from = abs_pos + 1;
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
    fn reports_chained_not() {
        let d = SelectorNotNotation.check(&style_with_selector(":not(.a):not(.b)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("chained :not()"));
    }

    #[test]
    fn allows_list_not() {
        let d = SelectorNotNotation.check(&style_with_selector(":not(.a, .b)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_single_not() {
        let d = SelectorNotNotation.check(&style_with_selector(":not(.a)"), &ctx());
        assert!(d.is_empty());
    }
}
