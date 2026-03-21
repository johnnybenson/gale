use std::collections::HashMap;

use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, SourceLineIndex, Span};

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

    fn check_root(&self, nodes: &[CssNode], context: &RuleContext) -> Vec<Diagnostic> {
        let line_index = SourceLineIndex::build(context.source);
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut diagnostics = Vec::new();

        collect_selectors(nodes, &mut seen, &mut diagnostics, self, &line_index, context.source);

        diagnostics
    }
}

fn collect_selectors(
    nodes: &[CssNode],
    seen: &mut HashMap<String, usize>,
    diagnostics: &mut Vec<Diagnostic>,
    rule: &NoDuplicateSelectors,
    line_index: &SourceLineIndex,
    source: &str,
) {
    for node in nodes {
        match node {
            CssNode::Style(style_rule) => {
                // Stylelint skips selectors with SCSS/Less interpolation
                // (non-standard syntax).
                let raw = style_rule.selector.trim();
                if raw.contains("#{") || raw.contains("@{") {
                    continue;
                }
                let selector = normalize_selector(raw);

                // Use the original source text for the message to preserve
                // exact formatting (whitespace, quoting, etc.).
                let display_selector = {
                    let start = style_rule.span.offset;
                    let end = (start + style_rule.selector.len()).min(source.len());
                    if start < source.len() {
                        // Extract from source and trim to the selector
                        // (before the opening brace).
                        let src = &source[start..end];
                        let trimmed = src.trim();
                        if trimmed.is_empty() {
                            raw
                        } else {
                            trimmed
                        }
                    } else {
                        raw
                    }
                };

                if let Some(&first_line) = seen.get(&selector) {
                    diagnostics.push(
                        Diagnostic::new(
                            rule.name(),
                            format!(
                                "Unexpected duplicate selector \"{}\", first used at line {}",
                                display_selector,
                                first_line,
                            ),
                        )
                        .severity(rule.default_severity())
                        .span(Span::new(style_rule.span.offset, style_rule.span.length)),
                    );
                } else {
                    let (line, _) = line_index.offset_to_location(style_rule.span.offset);
                    seen.insert(selector, line);
                }
            }
            CssNode::AtRule(at_rule) => {
                // Use a separate scope for at-rule children so that selectors
                // inside different at-rules (e.g. @media) don't clash with
                // top-level selectors.
                let mut scoped_seen = HashMap::new();
                collect_selectors(
                    &at_rule.children,
                    &mut scoped_seen,
                    diagnostics,
                    rule,
                    line_index,
                    source,
                );
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
    fn reports_duplicate_selectors_with_line_reference() {
        let rule = NoDuplicateSelectors;
        let source = ".foo { color: red; }\n.bar { color: blue; }\n.foo { display: block; }";
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
                span: ParserSpan::new(0, 20),
            }),
            CssNode::Style(StyleRule {
                selector: ".bar".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "blue".to_string(),
                    span: ParserSpan::new(27, 11),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(21, 21),
            }),
            CssNode::Style(StyleRule {
                selector: ".foo".to_string(),
                declarations: vec![Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(49, 14),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(43, 24),
            }),
        ];
        let ctx = RuleContext {
            file_path: "test.css",
            source,
            syntax: Syntax::Css,
            options: None,
        };
        let diags = rule.check_root(&nodes, &ctx);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "Unexpected duplicate selector \".foo\", first used at line 1"
        );
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
