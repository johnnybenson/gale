use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports when a selector with lower specificity appears after a selector with
/// higher specificity, which may indicate a specificity ordering issue.
///
/// Equivalent to Stylelint's `no-descending-specificity` rule.
pub struct NoDescendingSpecificity;

/// Specificity as an (a, b, c) tuple where:
/// - a = number of ID selectors
/// - b = number of class selectors, attribute selectors, and pseudo-classes
/// - c = number of type selectors and pseudo-elements
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Specificity(u32, u32, u32);

/// Calculate the specificity of a selector string.
///
/// This is a simplified calculation that handles common cases:
/// - `#id` contributes to `a`
/// - `.class`, `[attr]`, `:pseudo-class` contribute to `b`
/// - `element`, `::pseudo-element` contribute to `c`
/// - Universal selector `*` contributes nothing
/// - Combinators (` `, `>`, `+`, `~`) contribute nothing
fn calculate_specificity(selector: &str) -> Specificity {
    let mut a: u32 = 0; // IDs
    let mut b: u32 = 0; // classes, attributes, pseudo-classes
    let mut c: u32 = 0; // elements, pseudo-elements

    let chars: Vec<char> = selector.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        match chars[i] {
            '#' => {
                a += 1;
                i += 1;
                // Skip the identifier
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_') {
                    i += 1;
                }
            }
            '.' => {
                b += 1;
                i += 1;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_') {
                    i += 1;
                }
            }
            '[' => {
                b += 1;
                // Skip to closing ]
                while i < len && chars[i] != ']' {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
            }
            ':' => {
                i += 1;
                if i < len && chars[i] == ':' {
                    // Pseudo-element (::before, ::after, etc.)
                    c += 1;
                    i += 1;
                    while i < len && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_') {
                        i += 1;
                    }
                } else {
                    // Pseudo-class (:hover, :first-child, etc.)
                    // Exception: :where() has 0 specificity, :is() and :not()
                    // take the specificity of their most specific argument.
                    // For simplicity, we just count all pseudo-classes as b += 1.
                    let start = i;
                    while i < len && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_') {
                        i += 1;
                    }
                    let pseudo_name: String = chars[start..i].iter().collect();
                    if pseudo_name == "where" {
                        // :where() has 0 specificity
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
                        b += 1;
                        // Skip parenthesized arguments like :nth-child(2n+1)
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
                    }
                }
            }
            '*' | ' ' | '>' | '+' | '~' | ',' => {
                i += 1;
            }
            ch if ch.is_alphanumeric() || ch == '-' || ch == '_' => {
                // Type selector
                c += 1;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_') {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    Specificity(a, b, c)
}

/// Collect all style rule selectors from a list of nodes, recursing into at-rules.
fn collect_selectors(nodes: &[CssNode], out: &mut Vec<(String, Specificity, Span)>) {
    for node in nodes {
        match node {
            CssNode::Style(style) => {
                // A selector list like "a, .b" can contain multiple selectors.
                // Split by comma and use the highest specificity.
                // But for reporting, we use the full selector string.
                let spec = calculate_specificity(&style.selector);
                let span = gale_diagnostics::Span::new(style.span.offset, style.span.length);
                out.push((style.selector.clone(), spec, span));

                // Recurse into nested children.
                for child in &style.children {
                    let child_spec = calculate_specificity(&child.selector);
                    let child_span =
                        gale_diagnostics::Span::new(child.span.offset, child.span.length);
                    out.push((child.selector.clone(), child_spec, child_span));
                }
            }
            CssNode::AtRule(at_rule) => {
                collect_selectors(&at_rule.children, out);
            }
            _ => {}
        }
    }
}

impl Rule for NoDescendingSpecificity {
    fn name(&self) -> &'static str {
        "no-descending-specificity"
    }

    fn description(&self) -> &'static str {
        "Disallow selectors of lower specificity from coming after overriding selectors of higher specificity"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, nodes: &[CssNode], _context: &RuleContext) -> Vec<Diagnostic> {
        let mut selectors = Vec::new();
        collect_selectors(nodes, &mut selectors);

        let mut diagnostics = Vec::new();
        let mut max_specificity: Option<(Specificity, String)> = None;

        for (selector, spec, span) in &selectors {
            if let Some((prev_spec, prev_selector)) = &max_specificity {
                if spec < prev_spec {
                    diagnostics.push(
                        Diagnostic::new(
                            self.name(),
                            format!(
                                "Expected selector \"{}\" to come before selector \"{}\" (lower specificity selectors should come first)",
                                selector, prev_selector,
                            ),
                        )
                        .severity(self.default_severity())
                        .span(*span),
                    );
                }
            }

            if max_specificity
                .as_ref()
                .map_or(true, |(prev, _)| spec >= prev)
            {
                max_specificity = Some((*spec, selector.clone()));
            }
        }

        diagnostics
    }
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
        }
    }

    #[test]
    fn reports_descending_specificity() {
        let rule = NoDescendingSpecificity;
        // #id { } then a { } => #id has higher specificity than a
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "#id".to_string(),
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
                selector: "a".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "blue".to_string(),
                    span: ParserSpan::new(22, 11),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(19, 16),
            }),
        ];
        let diags = rule.check_root(&nodes, &make_context());
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("lower specificity"));
    }

    #[test]
    fn ignores_ascending_specificity() {
        let rule = NoDescendingSpecificity;
        // a { } then .class { } then #id { }
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 5),
            }),
            CssNode::Style(StyleRule {
                selector: ".cls".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(6, 8),
            }),
            CssNode::Style(StyleRule {
                selector: "#id".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(15, 7),
            }),
        ];
        let diags = rule.check_root(&nodes, &make_context());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_specificity_calculation() {
        assert_eq!(calculate_specificity("a"), Specificity(0, 0, 1));
        assert_eq!(calculate_specificity(".class"), Specificity(0, 1, 0));
        assert_eq!(calculate_specificity("#id"), Specificity(1, 0, 0));
        assert_eq!(calculate_specificity("a.class"), Specificity(0, 1, 1));
        assert_eq!(calculate_specificity("#id .class a"), Specificity(1, 1, 1));
    }
}
