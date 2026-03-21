use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Limit nesting depth of CSS rules.
///
/// Equivalent to Stylelint's `max-nesting-depth` rule.
/// Default maximum: 3.
pub struct MaxNestingDepth;

const MAX_DEPTH: usize = 3;

impl Rule for MaxNestingDepth {
    fn name(&self) -> &'static str {
        "max-nesting-depth"
    }

    fn description(&self) -> &'static str {
        "Limit nesting depth"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, nodes: &[CssNode], _ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for node in nodes {
            match node {
                CssNode::Style(rule) => {
                    check_style_depth(self, rule, 1, &mut diags);
                }
                CssNode::AtRule(at_rule) => {
                    for child in &at_rule.children {
                        if let CssNode::Style(rule) = child {
                            check_style_depth(self, rule, 1, &mut diags);
                        }
                    }
                }
                _ => {}
            }
        }
        diags
    }
}

fn check_style_depth(
    rule_impl: &MaxNestingDepth,
    style: &gale_css_parser::StyleRule,
    depth: usize,
    diags: &mut Vec<Diagnostic>,
) {
    for child in &style.children {
        if depth > MAX_DEPTH {
            diags.push(
                Diagnostic::new(
                    rule_impl.name(),
                    format!(
                        "Expected nesting depth to be no more than {MAX_DEPTH}, found {depth}"
                    ),
                )
                .severity(rule_impl.default_severity())
                .span(Span::new(child.span.offset, child.span.length)),
            );
        }
        check_style_depth(rule_impl, child, depth + 1, diags);
    }
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

    fn make_decl() -> Declaration {
        Declaration {
            property: "color".to_string(),
            value: "red".to_string(),
            span: ParserSpan::new(0, 0),
            important: false,
        }
    }

    fn make_nested(depth: usize) -> StyleRule {
        if depth == 0 {
            return StyleRule {
                selector: ".leaf".to_string(),
                declarations: vec![make_decl()],
                children: vec![],
                span: ParserSpan::new(0, 0),
            };
        }
        StyleRule {
            selector: format!(".level-{depth}"),
            declarations: vec![make_decl()],
            children: vec![make_nested(depth - 1)],
            span: ParserSpan::new(0, 0),
        }
    }

    #[test]
    fn reports_deep_nesting() {
        // depth 5: .level-5 > .level-4 > .level-3 > .level-2 > .level-1 > .leaf
        let root = CssNode::Style(make_nested(5));
        let d = MaxNestingDepth.check_root(&[root], &ctx());
        assert!(!d.is_empty(), "expected diagnostics for deep nesting");
    }

    #[test]
    fn allows_shallow_nesting() {
        // depth 2: .level-2 > .level-1 > .leaf
        let root = CssNode::Style(make_nested(2));
        let d = MaxNestingDepth.check_root(&[root], &ctx());
        assert!(d.is_empty());
    }
}
