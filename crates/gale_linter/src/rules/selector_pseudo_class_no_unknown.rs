use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::data::is_known_pseudo_class;
use crate::rule::{Rule, RuleContext};

pub struct SelectorPseudoClassNoUnknown;

impl Rule for SelectorPseudoClassNoUnknown {
    fn name(&self) -> &'static str {
        "selector-pseudo-class-no-unknown"
    }

    fn description(&self) -> &'static str {
        "Disallow unknown pseudo-class selectors"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let mut diags = Vec::new();
        for name in extract_pseudo_classes(&rule.selector) {
            if name.starts_with('-') {
                continue; // vendor-prefixed
            }
            // Legacy pseudo-elements that use single-colon syntax are not
            // pseudo-classes and must not be reported as unknown.
            if matches!(
                name.as_str(),
                "before" | "after" | "first-line" | "first-letter"
            ) {
                continue;
            }
            if !is_known_pseudo_class(&name) {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!("Unexpected unknown pseudo-class selector \":{name}\""),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(rule.span.offset, rule.span.length)),
                );
            }
        }
        diags
    }
}

/// Extract pseudo-class names from a selector string.
/// Finds `:name` patterns (single colon NOT followed by another colon).
fn extract_pseudo_classes(selector: &str) -> Vec<String> {
    let mut classes = Vec::new();
    let chars: Vec<char> = selector.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Skip pseudo-elements (::name)
        if i + 1 < len && chars[i] == ':' && chars[i + 1] == ':' {
            i += 2; // skip ::
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '-') {
                i += 1;
            }
            continue;
        }
        if chars[i] == ':' {
            i += 1; // skip the colon
            let start = i;
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '-') {
                i += 1;
            }
            if i > start {
                let name: String = chars[start..i].iter().collect();
                classes.push(name);
            }
            // Skip past any parenthesized argument like :nth-child(2)
            if i < len && chars[i] == '(' {
                let mut depth = 1;
                i += 1;
                while i < len && depth > 0 {
                    if chars[i] == '(' {
                        depth += 1;
                    } else if chars[i] == ')' {
                        depth -= 1;
                    }
                    i += 1;
                }
            }
        } else {
            i += 1;
        }
    }

    classes
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{CssNode, Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css }
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
    fn reports_unknown_pseudo_class() {
        let d = SelectorPseudoClassNoUnknown.check(&style_with_selector("a:hoverr"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains(":hoverr"));
    }

    #[test]
    fn allows_known_pseudo_class() {
        assert!(SelectorPseudoClassNoUnknown.check(&style_with_selector("a:hover"), &ctx()).is_empty());
        assert!(SelectorPseudoClassNoUnknown.check(&style_with_selector("a:nth-child(2)"), &ctx()).is_empty());
    }

    #[test]
    fn does_not_confuse_with_pseudo_elements() {
        // ::before should not be parsed as pseudo-class
        assert!(SelectorPseudoClassNoUnknown.check(&style_with_selector("a::before"), &ctx()).is_empty());
    }

    #[test]
    fn allows_legacy_single_colon_pseudo_elements() {
        for sel in [
            "p:before",
            "p:after",
            "p:first-line",
            "p:first-letter",
        ] {
            assert!(
                SelectorPseudoClassNoUnknown.check(&style_with_selector(sel), &ctx()).is_empty(),
                "should not flag legacy pseudo-element in selector \"{sel}\"",
            );
        }
    }
}
