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

/// Extract the content inside balanced parentheses, advancing `i` past the
/// closing `)`.  Assumes `chars[*i] == '('` on entry.
fn extract_parenthesized_content(chars: &[char], i: &mut usize) -> String {
    let mut depth = 1;
    *i += 1; // skip opening '('
    let start = *i;
    while *i < chars.len() && depth > 0 {
        if chars[*i] == '(' {
            depth += 1;
        } else if chars[*i] == ')' {
            depth -= 1;
        }
        if depth > 0 {
            *i += 1;
        }
    }
    let content: String = chars[start..*i].iter().collect();
    if *i < chars.len() {
        *i += 1; // skip closing ')'
    }
    content
}

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
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_')
                {
                    i += 1;
                }
            }
            '.' => {
                b += 1;
                i += 1;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_')
                {
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
                    while i < len
                        && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_')
                    {
                        i += 1;
                    }
                } else {
                    // Pseudo-class (:hover, :first-child, etc.)
                    // Special handling per CSS Selectors Level 4:
                    // - :where() has zero specificity
                    // - :is(), :not(), :has() take the specificity of their
                    //   most specific argument
                    let start = i;
                    while i < len
                        && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_')
                    {
                        i += 1;
                    }
                    let pseudo_name: String = chars[start..i].iter().collect();
                    if pseudo_name == "where" {
                        // :where() has 0 specificity -- skip arguments entirely
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
                    } else if pseudo_name == "is" || pseudo_name == "not" || pseudo_name == "has" {
                        // :is(), :not(), :has() take the specificity of the
                        // most specific argument (comma-separated selectors).
                        if i < len && chars[i] == '(' {
                            let inner = extract_parenthesized_content(&chars, &mut i);
                            let mut max_arg = Specificity(0, 0, 0);
                            for arg in inner.split(',') {
                                let arg_spec = calculate_specificity(arg.trim());
                                max_arg = max_arg.max(arg_spec);
                            }
                            a += max_arg.0;
                            b += max_arg.1;
                            c += max_arg.2;
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
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_')
                {
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

/// Walk nodes and check specificity in a single pass (O(n)), avoiding intermediate allocation.
fn check_specificity_walk(
    nodes: &[CssNode],
    max_specificity: &mut Option<(Specificity, String)>,
    diagnostics: &mut Vec<Diagnostic>,
    rule: &NoDescendingSpecificity,
) {
    for node in nodes {
        match node {
            CssNode::Style(style) => {
                check_one_selector(
                    &style.selector,
                    gale_diagnostics::Span::new(style.span.offset, style.span.length),
                    max_specificity,
                    diagnostics,
                    rule,
                );

                // Recurse into nested children.
                for child in &style.children {
                    check_one_selector(
                        &child.selector,
                        gale_diagnostics::Span::new(child.span.offset, child.span.length),
                        max_specificity,
                        diagnostics,
                        rule,
                    );
                }
            }
            CssNode::AtRule(at_rule) => {
                check_specificity_walk(&at_rule.children, max_specificity, diagnostics, rule);
            }
            _ => {}
        }
    }
}

fn check_one_selector(
    selector: &str,
    span: Span,
    max_specificity: &mut Option<(Specificity, String)>,
    diagnostics: &mut Vec<Diagnostic>,
    rule: &NoDescendingSpecificity,
) {
    let spec = calculate_specificity(selector);

    if let Some((prev_spec, prev_selector)) = max_specificity.as_ref()
        && spec < *prev_spec
    {
        diagnostics.push(
            Diagnostic::new(
                rule.name(),
                format!(
                    "Expected selector \"{}\" to come before selector \"{}\" (lower specificity selectors should come first)",
                    selector, prev_selector,
                ),
            )
            .severity(rule.default_severity())
            .span(span),
        );
    }

    if max_specificity
        .as_ref()
        .is_none_or(|(prev, _)| spec >= *prev)
    {
        *max_specificity = Some((spec, selector.to_string()));
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
        let mut diagnostics = Vec::new();
        let mut max_specificity: Option<(Specificity, String)> = None;

        check_specificity_walk(nodes, &mut max_specificity, &mut diagnostics, self);

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
            options: None,
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

    #[test]
    fn test_where_has_zero_specificity() {
        // :where() contributes nothing
        assert_eq!(calculate_specificity(":where(.a)"), Specificity(0, 0, 0));
        assert_eq!(calculate_specificity(":where(#id)"), Specificity(0, 0, 0));
        assert_eq!(calculate_specificity("a:where(.b)"), Specificity(0, 0, 1));
    }

    #[test]
    fn test_is_takes_max_argument_specificity() {
        // :is(.a) has specificity of .a => (0,1,0)
        assert_eq!(calculate_specificity(":is(.a)"), Specificity(0, 1, 0));
        // :is(#id, .a) takes the max => (1,0,0)
        assert_eq!(calculate_specificity(":is(#id, .a)"), Specificity(1, 0, 0));
        // a:is(.b) => type(a) + class(.b) = (0,1,1)
        assert_eq!(calculate_specificity("a:is(.b)"), Specificity(0, 1, 1));
    }

    #[test]
    fn test_not_takes_max_argument_specificity() {
        assert_eq!(calculate_specificity(":not(.a)"), Specificity(0, 1, 0));
        assert_eq!(calculate_specificity(":not(#id)"), Specificity(1, 0, 0));
    }

    #[test]
    fn test_has_takes_max_argument_specificity() {
        assert_eq!(calculate_specificity(":has(.a)"), Specificity(0, 1, 0));
        assert_eq!(calculate_specificity(":has(> .a)"), Specificity(0, 1, 0));
    }
}
