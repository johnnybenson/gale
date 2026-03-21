use std::collections::HashSet;

use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow duplicate selectors within a stylesheet.
///
/// Equivalent to Stylelint's `no-duplicate-selectors` rule.
pub struct NoDuplicateSelectors;

impl Rule for NoDuplicateSelectors {
    fn name(&self) -> &'static str {
        "no-duplicate-selectors"
    }

    fn description(&self) -> &'static str {
        "Disallow duplicate selectors within a stylesheet"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, nodes: &[CssNode], _context: &RuleContext) -> Vec<Diagnostic> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut diagnostics = Vec::new();

        collect_selectors(nodes, &mut seen, &mut diagnostics, self);

        diagnostics
    }
}

fn collect_selectors(
    nodes: &[CssNode],
    seen: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    rule: &NoDuplicateSelectors,
) {
    for node in nodes {
        match node {
            CssNode::Style(style_rule) => {
                let selector = normalize_selector(style_rule.selector.trim());
                if seen.contains(&selector) {
                    diagnostics.push(
                        Diagnostic::new(
                            rule.name(),
                            format!(
                                "Unexpected duplicate selector \"{}\"",
                                style_rule.selector.trim()
                            ),
                        )
                        .severity(rule.default_severity())
                        .span(Span::new(style_rule.span.offset, style_rule.span.length)),
                    );
                } else {
                    seen.insert(selector);
                }
            }
            CssNode::AtRule(at_rule) => {
                // Use a separate scope for at-rule children so that selectors
                // inside different at-rules (e.g. @media) don't clash with
                // top-level selectors.
                let mut scoped_seen = HashSet::new();
                collect_selectors(&at_rule.children, &mut scoped_seen, diagnostics, rule);
            }
            _ => {}
        }
    }
}

/// Normalize a selector for duplicate comparison.
///
/// Sorts comma-separated selector lists so that `a, b` and `b, a` are
/// considered the same. Also trims whitespace around each part.
fn normalize_selector(selector: &str) -> String {
    let mut parts: Vec<String> = selector
        .split(',')
        .map(|s| collapse_whitespace(s.trim()))
        .collect();
    parts.sort_unstable();
    parts.join(", ")
}

/// Collapse runs of whitespace into a single space.
fn collapse_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_was_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            result.push(ch);
            prev_was_space = false;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn make_context() -> RuleContext<'static> {
        RuleContext {
            file_path: "test.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        }
    }

    #[test]
    fn reports_duplicate_selectors() {
        let rule = NoDuplicateSelectors;
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: ".foo".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(6, 10),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(0, 18),
            }),
            CssNode::Style(StyleRule {
                selector: ".bar".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "blue".to_string(),
                    span: ParserSpan::new(25, 11),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(19, 19),
            }),
            CssNode::Style(StyleRule {
                selector: ".foo".to_string(),
                declarations: vec![Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(45, 14),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(39, 22),
            }),
        ];
        let diags = rule.check_root(&nodes, &make_context());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "Unexpected duplicate selector \".foo\"");
    }

    #[test]
    fn ignores_unique_selectors() {
        let rule = NoDuplicateSelectors;
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: ".foo".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 10),
            }),
            CssNode::Style(StyleRule {
                selector: ".bar".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(11, 10),
            }),
        ];
        let diags = rule.check_root(&nodes, &make_context());
        assert!(diags.is_empty());
    }

    #[test]
    fn scoped_selectors_in_at_rules_dont_clash() {
        let rule = NoDuplicateSelectors;
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: ".foo".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 10),
            }),
            CssNode::AtRule(gale_css_parser::AtRule {
                name: "media".to_string(),
                params: "(min-width: 768px)".to_string(),
                span: ParserSpan::new(11, 50),
                children: vec![CssNode::Style(StyleRule {
                    selector: ".foo".to_string(),
                    declarations: vec![],
                    children: vec![],
                    span: ParserSpan::new(30, 10),
                })],
            }),
        ];
        let diags = rule.check_root(&nodes, &make_context());
        assert!(
            diags.is_empty(),
            "same selector in different scopes should not be flagged"
        );
    }

    #[test]
    fn detects_duplicates_in_parsed_scss() {
        let scss = ".foo { color: red; }\n.bar { color: blue; }\n.foo { display: block; }";
        let result = gale_css_parser::parse(scss, Syntax::Scss).expect("should parse SCSS");
        let rule = NoDuplicateSelectors;
        let ctx = RuleContext {
            file_path: "test.scss",
            source: scss,
            syntax: Syntax::Scss,
            options: None,
        };
        let diags = rule.check_root(&result.nodes, &ctx);
        assert_eq!(diags.len(), 1, "should detect duplicate .foo in SCSS");
    }
}
