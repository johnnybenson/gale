use gale_css_parser::{CssNode, Syntax};
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow redundant nesting selectors (`&`).
///
/// Reports when `&` appears at the start of a nested selector followed by a
/// combinator or descendant selector, because the `&` can be removed without
/// changing the meaning.
///
/// A standalone `& { ... }` is **not** flagged — it serves as a scoping
/// mechanism (e.g. after `@include` or `@extend`).
///
/// ```scss
/// // Bad
/// .foo {
///   & .bar { color: red; }   // → `.bar { color: red; }`
///   & > .bar { color: red; } // → `> .bar { color: red; }`
/// }
///
/// // Good
/// .foo {
///   & { color: red; }        // scoping block
///   &:hover { color: red; }  // pseudo-class append
///   &.bar { color: red; }    // class append
///   &__element { ... }       // BEM concatenation
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

        // Check nested children for redundant `&` selectors.
        //
        // A standalone `& { ... }` (where the entire selector is just `&`) is NOT
        // flagged.  In SCSS it serves as a scoping mechanism (e.g. after `@include`
        // or `@extend`), and Stylelint's scss/selector-no-redundant-nesting-selector
        // does not flag it either.
        //
        // We only flag `&` when it appears at the **start** of a compound selector
        // followed by a combinator or another simple selector — i.e. the `&` can be
        // removed without changing the meaning:
        //   `& .foo`  → `.foo`
        //   `& > .bar` → `> .bar`
        //   `& + .baz` → `+ .baz`
        //   `& ~ .qux` → `~ .qux`
        for child in &rule.children {
            let selector = child.selector.trim();

            // Skip standalone `&` — it's a scoping block, not redundant.
            if selector == "&" {
                continue;
            }

            // Flag `& <combinator-or-descendant> ...` patterns where the leading
            // `&` is unnecessary.
            if let Some(rest) = selector.strip_prefix('&') {
                let after = rest.trim_start();
                // After stripping `&` and whitespace the remainder must start with
                // a simple selector or combinator — i.e. the `&` was at the
                // beginning of a compound/complex selector.
                // If the very first char after `&` is NOT a combinator char, class
                // selector, id selector, pseudo-class/element, or attribute
                // selector, then the `&` was concatenated (e.g. `&__element`) which
                // is fine.
                let redundant = if rest.starts_with(char::is_whitespace) {
                    // `& .foo`, `& > .bar`, etc. — descendant / combinator after
                    // whitespace: `&` is redundant.
                    !after.is_empty()
                } else {
                    false
                };
                if redundant {
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
    fn allows_standalone_ampersand_block() {
        // A bare `& { ... }` is a scoping mechanism and should NOT be flagged.
        let node = nested_rule(".foo", "&");
        assert!(
            ScssSelectorNoRedundantNestingSelector
                .check(&node, &scss_ctx())
                .is_empty()
        );
    }

    #[test]
    fn reports_redundant_ampersand_descendant() {
        // `& .bar` is redundant — can be written as `.bar`.
        let node = nested_rule(".foo", "& .bar");
        let d = ScssSelectorNoRedundantNestingSelector.check(&node, &scss_ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("redundant"));
    }

    #[test]
    fn reports_redundant_ampersand_child_combinator() {
        // `& > .bar` is redundant — can be written as `> .bar`.
        let node = nested_rule(".foo", "& > .bar");
        let d = ScssSelectorNoRedundantNestingSelector.check(&node, &scss_ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("redundant"));
    }

    #[test]
    fn reports_redundant_ampersand_sibling_combinator() {
        // `& + .bar` is redundant — can be written as `+ .bar`.
        let node = nested_rule(".foo", "& + .bar");
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
    fn allows_ampersand_concatenation() {
        // `&__element` is BEM-style concatenation, NOT redundant.
        let node = nested_rule(".foo", "&__element");
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
