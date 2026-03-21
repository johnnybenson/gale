use std::collections::HashMap;

use gale_css_parser::{CssNode, Syntax};
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

/// Returns `true` if the selector contains SCSS/Less constructs that make
/// specificity analysis unreliable:
/// - `#{` — SCSS interpolation (dynamic specificity)
/// - `%`  — placeholder selectors (extended, not used directly)
/// - Leading `&` — parent-referencing selectors (compose with parent)
fn has_preprocessor_constructs(selector: &str) -> bool {
    let trimmed = selector.trim();
    trimmed.contains("#{") || trimmed.starts_with('%') || trimmed.starts_with('&')
}

/// Extract the last compound selector from a complex selector, stripping
/// pseudo-classes.  This is used to group selectors for comparison — only
/// selectors that share the same "last compound" can be meaningfully compared
/// for specificity ordering (matching Stylelint behaviour).
///
/// For example:
/// - `.foo .bar:hover` → `.bar`
/// - `#id > .cls` → `.cls`
/// - `a` → `a`
/// - `a, b` → last individual selector's last compound
fn last_compound_selector_without_pseudo_classes(selector: &str) -> String {
    // For comma-separated selector lists, take the last individual selector.
    let individual = selector
        .rsplit(',')
        .next()
        .unwrap_or(selector)
        .trim();

    // Split by combinators (` `, `>`, `+`, `~`) to get the last compound.
    // We walk backwards to find the last combinator.
    let chars: Vec<char> = individual.chars().collect();
    let mut last_compound_start = 0;
    let mut i = chars.len();
    let mut in_parens = 0i32;
    while i > 0 {
        i -= 1;
        match chars[i] {
            ')' => in_parens += 1,
            '(' => in_parens -= 1,
            ' ' | '>' | '+' | '~' if in_parens == 0 => {
                // Found a combinator; the last compound starts after it
                // (skip any whitespace/combinators).
                let mut j = i + 1;
                while j < chars.len()
                    && matches!(chars[j], ' ' | '>' | '+' | '~')
                {
                    j += 1;
                }
                last_compound_start = j;
                break;
            }
            _ => {}
        }
    }

    let compound: String = chars[last_compound_start..].iter().collect();

    // Strip pseudo-classes (`:name` but not `::pseudo-element`)
    let mut result = String::new();
    let compound_chars: Vec<char> = compound.chars().collect();
    let clen = compound_chars.len();
    let mut ci = 0;
    while ci < clen {
        if compound_chars[ci] == ':' && ci + 1 < clen && compound_chars[ci + 1] != ':' {
            // This is a pseudo-class; skip it.
            ci += 1; // skip ':'
            // Skip the pseudo-class name
            if ci < clen && compound_chars[ci] == '(' {
                // functional pseudo-class like :not(...)
                let mut depth = 1;
                ci += 1;
                while ci < clen && depth > 0 {
                    if compound_chars[ci] == '(' {
                        depth += 1;
                    } else if compound_chars[ci] == ')' {
                        depth -= 1;
                    }
                    ci += 1;
                }
            } else {
                while ci < clen
                    && (compound_chars[ci].is_alphanumeric()
                        || compound_chars[ci] == '-'
                        || compound_chars[ci] == '_')
                {
                    ci += 1;
                }
                // Handle functional pseudo-class: :nth-child(...)
                if ci < clen && compound_chars[ci] == '(' {
                    let mut depth = 1;
                    ci += 1;
                    while ci < clen && depth > 0 {
                        if compound_chars[ci] == '(' {
                            depth += 1;
                        } else if compound_chars[ci] == ')' {
                            depth -= 1;
                        }
                        ci += 1;
                    }
                }
            }
        } else {
            result.push(compound_chars[ci]);
            ci += 1;
        }
    }

    result.to_lowercase()
}

/// Walk nodes and check specificity, grouping selectors by their last
/// compound selector (without pseudo-classes) so that only selectors
/// targeting the same element are compared.  This matches Stylelint's
/// comparison strategy.
///
/// When `top_level_only` is true, only top-level style rules are compared
/// (no recursion into nested children).  This is used for SCSS/Sass/Less
/// where nested selectors compose with their parent so their written
/// specificity is incomplete.
fn check_specificity_walk(
    nodes: &[CssNode],
    comparison_ctx: &mut HashMap<String, (Specificity, String)>,
    diagnostics: &mut Vec<Diagnostic>,
    rule: &NoDescendingSpecificity,
    top_level_only: bool,
    skip_preprocessor: bool,
) {
    for node in nodes {
        match node {
            CssNode::Style(style) => {
                if !skip_preprocessor || !has_preprocessor_constructs(&style.selector) {
                    check_one_selector(
                        &style.selector,
                        gale_diagnostics::Span::new(style.span.offset, style.span.length),
                        comparison_ctx,
                        diagnostics,
                        rule,
                    );
                }

                // Recurse into nested children only for plain CSS.
                if !top_level_only {
                    for child in &style.children {
                        check_one_selector(
                            &child.selector,
                            gale_diagnostics::Span::new(child.span.offset, child.span.length),
                            comparison_ctx,
                            diagnostics,
                            rule,
                        );
                    }
                }
            }
            CssNode::AtRule(at_rule) => {
                check_specificity_walk(
                    &at_rule.children,
                    comparison_ctx,
                    diagnostics,
                    rule,
                    top_level_only,
                    skip_preprocessor,
                );
            }
            _ => {}
        }
    }
}

fn check_one_selector(
    selector: &str,
    span: Span,
    comparison_ctx: &mut HashMap<String, (Specificity, String)>,
    diagnostics: &mut Vec<Diagnostic>,
    rule: &NoDescendingSpecificity,
) {
    // For selector lists (comma-separated), check each individual selector.
    for individual in selector.split(',') {
        let individual = individual.trim();
        if individual.is_empty() {
            continue;
        }

        let spec = calculate_specificity(individual);
        let key = last_compound_selector_without_pseudo_classes(individual);

        if key.is_empty() {
            continue;
        }

        if let Some((prev_spec, prev_selector)) = comparison_ctx.get(&key) {
            if spec < *prev_spec {
                diagnostics.push(
                    Diagnostic::new(
                        rule.name(),
                        format!(
                            "Expected selector \"{}\" to come before selector \"{}\" (lower specificity selectors should come first)",
                            selector.trim(), prev_selector,
                        ),
                    )
                    .severity(rule.default_severity())
                    .span(span),
                );
            }
        }

        // Update the max specificity for this key.
        comparison_ctx
            .entry(key)
            .and_modify(|(prev_spec, prev_sel)| {
                if spec >= *prev_spec {
                    *prev_spec = spec;
                    *prev_sel = individual.to_string();
                }
            })
            .or_insert((spec, individual.to_string()));
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

    fn check_root(&self, nodes: &[CssNode], context: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut comparison_ctx: HashMap<String, (Specificity, String)> = HashMap::new();

        let is_preprocessor =
            matches!(context.syntax, Syntax::Scss | Syntax::Sass | Syntax::Less);

        // For SCSS/Sass/Less: only compare top-level selectors (nested selectors
        // compose with their parent so their written specificity is incomplete),
        // and skip selectors with preprocessor constructs (`#{`, `%`, `&`).
        // For plain CSS: full behavior — compare all selectors at all levels.
        check_specificity_walk(
            nodes,
            &mut comparison_ctx,
            &mut diagnostics,
            self,
            is_preprocessor, // top_level_only
            is_preprocessor, // skip_preprocessor
        );

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
        // `.foo .bar` then `.bar` => same last compound (`.bar`), descending
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: ".foo .bar".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(6, 10),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(0, 22),
            }),
            CssNode::Style(StyleRule {
                selector: ".bar".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "blue".to_string(),
                    span: ParserSpan::new(26, 11),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(23, 16),
            }),
        ];
        let diags = rule.check_root(&nodes, &make_context());
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("lower specificity"));
    }

    #[test]
    fn no_report_for_different_last_compound() {
        let rule = NoDescendingSpecificity;
        // `#id` then `a` => different last compound selectors, not comparable
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "#id".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 18),
            }),
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(19, 16),
            }),
        ];
        let diags = rule.check_root(&nodes, &make_context());
        assert!(diags.is_empty(), "different last compound selectors should not be compared");
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

    fn make_scss_context() -> RuleContext<'static> {
        RuleContext {
            file_path: "test.scss",
            source: "",
            syntax: Syntax::Scss,
            options: None,
        }
    }

    #[test]
    fn scss_reports_top_level_descending_specificity() {
        let rule = NoDescendingSpecificity;
        // Top-level selectors in SCSS with same last compound — should be checked
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: ".foo .bar".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(6, 10),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(0, 22),
            }),
            CssNode::Style(StyleRule {
                selector: ".bar".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "blue".to_string(),
                    span: ParserSpan::new(26, 11),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(23, 16),
            }),
        ];
        let diags = rule.check_root(&nodes, &make_scss_context());
        assert_eq!(diags.len(), 1, "should report descending specificity for top-level SCSS selectors");
    }

    #[test]
    fn scss_skips_nested_children() {
        let rule = NoDescendingSpecificity;
        // A top-level `.foo .bar` with a nested `.bar` child — the nested `.bar`
        // should NOT be compared in SCSS because nested selectors compose with parent.
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: ".foo .bar".to_string(),
                declarations: vec![],
                children: vec![StyleRule {
                    selector: ".bar".to_string(),
                    declarations: vec![Declaration {
                        property: "color".to_string(),
                        value: "blue".to_string(),
                        span: ParserSpan::new(12, 11),
                        important: false,
                    }],
                    children: vec![],
                    span: ParserSpan::new(8, 17),
                }],
                span: ParserSpan::new(0, 27),
            }),
        ];
        let diags = rule.check_root(&nodes, &make_scss_context());
        assert!(diags.is_empty(), "should not compare nested SCSS children against parent");

        // Same structure in plain CSS SHOULD compare nested children
        // (both share last compound `.bar`)
        let diags = rule.check_root(&nodes, &make_context());
        assert_eq!(diags.len(), 1, "plain CSS should compare nested children");
    }

    #[test]
    fn scss_skips_interpolation_selectors() {
        let rule = NoDescendingSpecificity;
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "#id".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 10),
            }),
            CssNode::Style(StyleRule {
                selector: ".#{$var}".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(11, 15),
            }),
        ];
        let diags = rule.check_root(&nodes, &make_scss_context());
        assert!(diags.is_empty(), "should skip selectors with SCSS interpolation");
    }

    #[test]
    fn scss_skips_ampersand_selectors() {
        let rule = NoDescendingSpecificity;
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "#id".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 10),
            }),
            CssNode::Style(StyleRule {
                selector: "&:hover".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(11, 15),
            }),
        ];
        let diags = rule.check_root(&nodes, &make_scss_context());
        assert!(diags.is_empty(), "should skip &-prefixed selectors in SCSS");
    }

    #[test]
    fn scss_skips_placeholder_selectors() {
        let rule = NoDescendingSpecificity;
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "#id".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 10),
            }),
            CssNode::Style(StyleRule {
                selector: "%placeholder".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(11, 20),
            }),
        ];
        let diags = rule.check_root(&nodes, &make_scss_context());
        assert!(diags.is_empty(), "should skip placeholder selectors in SCSS");
    }

    #[test]
    fn less_uses_same_selective_approach() {
        let rule = NoDescendingSpecificity;
        // Same last compound selector `.bar` — should report in Less
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: ".foo .bar".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 20),
            }),
            CssNode::Style(StyleRule {
                selector: ".bar".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(21, 12),
            }),
        ];
        let less_context = RuleContext {
            file_path: "test.less",
            source: "",
            syntax: Syntax::Less,
            options: None,
        };
        let diags = rule.check_root(&nodes, &less_context);
        assert_eq!(diags.len(), 1, "should report top-level descending specificity in Less");
    }
}
