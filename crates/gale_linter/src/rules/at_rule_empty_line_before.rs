use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require an empty line before at-rules (except first-nested and grouped imports).
///
/// Equivalent to Stylelint's `at-rule-empty-line-before` rule with "always" option.
/// Detection-only (no autofix).
pub struct AtRuleEmptyLineBefore;

/// At-rule names that are commonly grouped together without blank lines between them.
const GROUPABLE_AT_RULES: &[&str] = &["import", "use", "forward"];

impl Rule for AtRuleEmptyLineBefore {
    fn name(&self) -> &'static str {
        "at-rule-empty-line-before"
    }

    fn description(&self) -> &'static str {
        "Require an empty line before at-rules"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, nodes: &[CssNode], ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        check_at_rule_nodes(self, nodes, ctx, &mut diags);
        diags
    }
}

fn is_groupable(name: &str) -> bool {
    GROUPABLE_AT_RULES
        .iter()
        .any(|&g| name.eq_ignore_ascii_case(g))
}

fn prev_is_same_groupable_at_rule(nodes: &[CssNode], index: usize, current_name: &str) -> bool {
    if index == 0 || !is_groupable(current_name) {
        return false;
    }
    if let CssNode::AtRule(prev) = &nodes[index - 1] {
        is_groupable(&prev.name)
    } else {
        false
    }
}

fn check_at_rule_nodes(
    rule_impl: &AtRuleEmptyLineBefore,
    nodes: &[CssNode],
    ctx: &RuleContext,
    diags: &mut Vec<Diagnostic>,
) {
    // Read option for ignoring after-comment
    let ignore_after_comment = ctx
        .options
        .and_then(|v| v.get("ignore"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .any(|item| item.as_str() == Some("after-comment"))
        })
        .unwrap_or(false);

    for (i, node) in nodes.iter().enumerate() {
        if let CssNode::AtRule(at_rule) = node {
            // Skip the first node in a list (first-nested exception)
            if i == 0 {
                // Still recurse into children
                check_at_rule_nodes(rule_impl, &at_rule.children, ctx, diags);
                continue;
            }

            // Skip if this is a groupable at-rule preceded by another groupable at-rule
            if prev_is_same_groupable_at_rule(nodes, i, &at_rule.name) {
                check_at_rule_nodes(rule_impl, &at_rule.children, ctx, diags);
                continue;
            }

            // ignore: ["after-comment"] — skip if the previous node is a comment
            if ignore_after_comment && matches!(nodes[i - 1], CssNode::Comment(_)) {
                check_at_rule_nodes(rule_impl, &at_rule.children, ctx, diags);
                continue;
            }

            let offset = at_rule.span.offset;
            if offset > 0 && offset <= ctx.source.len() {
                let before = &ctx.source[..offset];

                // Source-level first-nested check: if the at-rule immediately
                // follows an opening brace `{` (after whitespace/newlines),
                // it's the first thing in a block and should be skipped.
                let trimmed_all_ws = before.trim();
                if trimmed_all_ws.ends_with('{') {
                    check_at_rule_nodes(rule_impl, &at_rule.children, ctx, diags);
                    continue;
                }

                let trimmed = before.trim_end_matches([' ', '\t']);
                if !trimmed.ends_with("\n\n") && !trimmed.ends_with("\r\n\r\n") {
                    diags.push(
                        Diagnostic::new(
                            rule_impl.name(),
                            format!("Expected empty line before at-rule \"@{}\"", at_rule.name),
                        )
                        .severity(rule_impl.default_severity())
                        .span(Span::new(at_rule.span.offset, at_rule.span.length)),
                    );
                }
            }

            // Recurse into children
            check_at_rule_nodes(rule_impl, &at_rule.children, ctx, diags);
        }

        // Also recurse into style rules to find nested at-rules
        if let CssNode::Style(style) = node {
            let child_nodes: Vec<CssNode> = style
                .children
                .iter()
                .map(|sr| CssNode::Style(sr.clone()))
                .collect();
            check_at_rule_nodes(rule_impl, &child_nodes, ctx, diags);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{AtRule as ParserAtRule, Span as ParserSpan, StyleRule, Syntax};

    fn make_ctx(source: &str) -> RuleContext {
        RuleContext {
            file_path: "t.css",
            source,
            syntax: Syntax::Css,
            options: None,
        }
    }

    #[test]
    fn reports_missing_empty_line_before_at_rule() {
        let src = "a { color: red; }\n@media screen { }";
        let at_offset = src.find("@media").unwrap();
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 17),
            }),
            CssNode::AtRule(ParserAtRule {
                name: "media".to_string(),
                params: "screen".to_string(),
                span: ParserSpan::new(at_offset, 18),
                children: vec![],
            }),
        ];
        let d = AtRuleEmptyLineBefore.check_root(&nodes, &make_ctx(src));
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("@media"));
    }

    #[test]
    fn allows_empty_line_before_at_rule() {
        let src = "a { color: red; }\n\n@media screen { }";
        let at_offset = src.find("@media").unwrap();
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 17),
            }),
            CssNode::AtRule(ParserAtRule {
                name: "media".to_string(),
                params: "screen".to_string(),
                span: ParserSpan::new(at_offset, 18),
                children: vec![],
            }),
        ];
        let d = AtRuleEmptyLineBefore.check_root(&nodes, &make_ctx(src));
        assert!(d.is_empty());
    }

    #[test]
    fn skips_first_nested_at_rule() {
        let src = "@media screen { }";
        let nodes = vec![CssNode::AtRule(ParserAtRule {
            name: "media".to_string(),
            params: "screen".to_string(),
            span: ParserSpan::new(0, src.len()),
            children: vec![],
        })];
        let d = AtRuleEmptyLineBefore.check_root(&nodes, &make_ctx(src));
        assert!(d.is_empty());
    }

    #[test]
    fn allows_grouped_imports_without_empty_line() {
        let src = "@import \"a.css\";\n@import \"b.css\";";
        let second_offset = src.rfind("@import").unwrap();
        let nodes = vec![
            CssNode::AtRule(ParserAtRule {
                name: "import".to_string(),
                params: "\"a.css\"".to_string(),
                span: ParserSpan::new(0, 16),
                children: vec![],
            }),
            CssNode::AtRule(ParserAtRule {
                name: "import".to_string(),
                params: "\"b.css\"".to_string(),
                span: ParserSpan::new(second_offset, 16),
                children: vec![],
            }),
        ];
        let d = AtRuleEmptyLineBefore.check_root(&nodes, &make_ctx(src));
        assert!(d.is_empty());
    }
}
