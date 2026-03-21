use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Edit, Fix, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Enforces double-colon `::` notation for pseudo-elements that also support
/// the legacy single-colon syntax (`:before`, `:after`, `:first-line`, `:first-letter`).
///
/// Equivalent to Stylelint's `selector-pseudo-element-colon-notation` rule.
pub struct SelectorPseudoElementColonNotation;

const LEGACY_PSEUDO_ELEMENTS: &[&str] = &["before", "after", "first-line", "first-letter"];

impl Rule for SelectorPseudoElementColonNotation {
    fn name(&self) -> &'static str {
        "selector-pseudo-element-colon-notation"
    }

    fn description(&self) -> &'static str {
        "Specify double colon notation for applicable pseudo-elements"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        // The parsed selector from lightningcss normalizes to `::`, so we must
        // look at the original source text to detect single-colon usage.
        let mut diags = Vec::new();

        // Find source text for this rule's selector area
        let source = ctx.source;
        if source.is_empty() {
            return vec![];
        }

        // Search for single-colon pseudo-elements in the source text around the
        // rule's span. We scan the source for `:name` that is NOT `::name`.
        let search_area = if rule.span.length > 0 {
            let end = (rule.span.offset + rule.span.length).min(source.len());
            &source[rule.span.offset..end]
        } else {
            // span length unknown; search whole source
            source
        };

        for pseudo in LEGACY_PSEUDO_ELEMENTS {
            let pattern = format!(":{pseudo}");
            let lower_search = search_area.to_ascii_lowercase();

            let mut pos = 0;
            while let Some(idx) = lower_search[pos..].find(&pattern) {
                let abs_idx = pos + idx;
                // Check it's not a double-colon
                let is_double = abs_idx > 0 && lower_search.as_bytes()[abs_idx - 1] == b':';
                // Check it's actually a pseudo-element boundary (next char after name is not alphanumeric)
                let end_idx = abs_idx + pattern.len();
                let at_boundary =
                    end_idx >= lower_search.len() || !lower_search.as_bytes()[end_idx].is_ascii_alphanumeric();

                if !is_double && at_boundary {
                    let fix_offset = rule.span.offset + abs_idx;
                    diags.push(
                        Diagnostic::new(
                            self.name(),
                            format!(
                                "Expected double colon notation \"::{pseudo}\" instead of \":{pseudo}\""
                            ),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(fix_offset, pattern.len()))
                        .fix(Fix::new(
                            format!("Replace :{pseudo} with ::{pseudo}"),
                            vec![Edit::new(
                                Span::new(fix_offset, pattern.len()),
                                format!("::{pseudo}"),
                            )],
                        )),
                    );
                    break;
                }
                pos = abs_idx + 1;
            }
        }

        // Deduplicate: also check the selector string (which is normalized) to avoid
        // false positives on selectors that don't use these pseudo-elements at all
        let sel_lower = rule.selector.to_ascii_lowercase();
        let has_relevant_pseudo = LEGACY_PSEUDO_ELEMENTS
            .iter()
            .any(|p| sel_lower.contains(&format!("::{p}")));
        if !has_relevant_pseudo {
            // If the normalized selector doesn't have these pseudo-elements,
            // they weren't pseudo-elements — clear diagnostics
            diags.clear();
        }

        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn style_with_selector_and_source(sel: &str, source: &'static str) -> (CssNode, RuleContext<'static>) {
        let node = CssNode::Style(StyleRule {
            selector: sel.to_string(),
            declarations: vec![Declaration {
                property: "color".to_string(),
                value: "red".to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, source.len()),
        });
        let ctx = RuleContext {
            file_path: "t.css",
            source,
            syntax: Syntax::Css, options: None };
        (node, ctx)
    }

    #[test]
    fn reports_single_colon_before() {
        // lightningcss normalizes selector to "a::before" but source has single colon
        let source = "a:before { color: red; }";
        let (node, ctx) = style_with_selector_and_source("a::before", source);
        let d = SelectorPseudoElementColonNotation.check(&node, &ctx);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("::before"));
    }

    #[test]
    fn allows_double_colon_before() {
        let source = "a::before { color: red; }";
        let (node, ctx) = style_with_selector_and_source("a::before", source);
        let d = SelectorPseudoElementColonNotation.check(&node, &ctx);
        assert!(d.is_empty());
    }

    #[test]
    fn reports_single_colon_after() {
        let source = "a:after { color: red; }";
        let (node, ctx) = style_with_selector_and_source("a::after", source);
        let d = SelectorPseudoElementColonNotation.check(&node, &ctx);
        assert_eq!(d.len(), 1);
    }
}
