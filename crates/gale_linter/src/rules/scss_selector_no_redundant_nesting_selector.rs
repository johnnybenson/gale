use gale_css_parser::{CssNode, Syntax};
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow redundant nesting selectors (`&`).
///
/// Reports when `&` is used as the entire selector in a nested rule, which is
/// redundant because the declarations could be placed in the parent rule.
///
/// ```scss
/// // Bad
/// .foo {
///   & { color: red; }
/// }
///
/// // Good
/// .foo {
///   &:hover { color: red; }
///   &.bar { color: red; }
/// }
/// ```
///
/// Equivalent to `scss/selector-no-redundant-nesting-selector`.
pub struct ScssSelectorNoRedundantNestingSelector;

impl Rule for ScssSelectorNoRedundantNestingSelector {
    fn name(&self) -> &'static str {
        "scss/selector-no-redundant-nesting-selector"
    }

    fn description(&self) -> &'static str {
        "Disallow redundant nesting selectors (&)"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        if !matches!(ctx.syntax, Syntax::Scss | Syntax::Sass) {
            return vec![];
        }

        let CssNode::Style(rule) = node else {
            return vec![];
        };

        let mut diags = Vec::new();

        // Check nested children for redundant `&` selectors
        for child in &rule.children {
            let selector = child.selector.trim();

            // The selector is just `&` with nothing else
            if selector == "&" {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        "Unexpected redundant nesting selector (&)".to_string(),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(child.span.offset, child.span.length)),
                );
            }
        }

        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn scss_ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "t.scss",
            source: "",
            syntax: Syntax::Scss,
            options: None,
        }
    }

    fn css_ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        }
    }

    fn nested_rule(parent_sel: &str, child_sel: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: parent_sel.to_string(),
            declarations: vec![],
            span: ParserSpan::new(0, 50),
            children: vec![StyleRule {
                selector: child_sel.to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(10, 10),
                    important: false,
                }],
                span: ParserSpan::new(5, 40),
                children: vec![],
            }],
        })
    }

    #[test]
    fn skips_non_scss() {
        let node = nested_rule(".foo", "&");
        assert!(
            ScssSelectorNoRedundantNestingSelector
                .check(&node, &css_ctx())
                .is_empty()
        );
    }

    #[test]
    fn reports_redundant_ampersand() {
        let node = nested_rule(".foo", "&");
        let d = ScssSelectorNoRedundantNestingSelector.check(&node, &scss_ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("redundant"));
    }

    #[test]
    fn allows_ampersand_with_pseudo() {
        let node = nested_rule(".foo", "&:hover");
        assert!(
            ScssSelectorNoRedundantNestingSelector
                .check(&node, &scss_ctx())
                .is_empty()
        );
    }

    #[test]
    fn allows_ampersand_with_class() {
        let node = nested_rule(".foo", "&.bar");
        assert!(
            ScssSelectorNoRedundantNestingSelector
                .check(&node, &scss_ctx())
                .is_empty()
        );
    }

    #[test]
    fn allows_descendant_selector() {
        let node = nested_rule(".foo", "& .bar");
        assert!(
            ScssSelectorNoRedundantNestingSelector
                .check(&node, &scss_ctx())
                .is_empty()
        );
    }

    #[test]
    fn allows_non_ampersand_selector() {
        let node = nested_rule(".foo", ".bar");
        assert!(
            ScssSelectorNoRedundantNestingSelector
                .check(&node, &scss_ctx())
                .is_empty()
        );
    }
}
