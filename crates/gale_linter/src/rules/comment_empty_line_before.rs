use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require an empty line before comments (except first-nested).
///
/// Equivalent to Stylelint's `comment-empty-line-before` rule with "always" option.
/// Detection-only (no autofix).
pub struct CommentEmptyLineBefore;

impl Rule for CommentEmptyLineBefore {
    fn name(&self) -> &'static str {
        "comment-empty-line-before"
    }

    fn description(&self) -> &'static str {
        "Require an empty line before comments"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, nodes: &[CssNode], ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        check_comment_nodes(self, nodes, ctx, &mut diags);
        diags
    }
}

/// Check whether a comment text contains a stylelint/gale command directive.
fn is_stylelint_command(text: &str) -> bool {
    let t = text.trim();
    t.starts_with("stylelint-disable")
        || t.starts_with("stylelint-enable")
        || t.starts_with("gale-disable")
        || t.starts_with("gale-enable")
}

fn check_comment_nodes(
    rule_impl: &CommentEmptyLineBefore,
    nodes: &[CssNode],
    ctx: &RuleContext,
    diags: &mut Vec<Diagnostic>,
) {
    // Read options for ignore/except configuration.
    let ignore_after_comment = ctx
        .options
        .and_then(|v| v.get("ignore"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().any(|item| item.as_str() == Some("after-comment")))
        .unwrap_or(false);

    let ignore_stylelint_commands = ctx
        .options
        .and_then(|v| v.get("ignore"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().any(|item| item.as_str() == Some("stylelint-commands")))
        .unwrap_or(false);

    for (i, node) in nodes.iter().enumerate() {
        if let CssNode::Comment(comment) = node {
            // In SCSS/Less/Sass, skip double-slash comments (`//`).
            if comment.is_line {
                continue;
            }

            // Skip the first node in a list (first-nested exception)
            if i == 0 {
                continue;
            }

            // ignore: ["stylelint-commands"] — skip stylelint/gale directive comments
            if ignore_stylelint_commands && is_stylelint_command(&comment.text) {
                continue;
            }

            // ignore: ["after-comment"] — skip if the previous node is a comment
            if ignore_after_comment && i > 0 {
                if matches!(nodes[i - 1], CssNode::Comment(_)) {
                    continue;
                }
            }

            let offset = comment.span.offset;
            if offset > 0 && offset <= ctx.source.len() {
                let before = &ctx.source[..offset];
                let trimmed = before.trim_end_matches([' ', '\t']);

                // Source-level first-nested check: if the comment immediately
                // follows an opening brace `{` (after whitespace/newlines),
                // it's the first thing in a block and should be skipped.
                let trimmed_all_ws = before.trim();
                if trimmed_all_ws.ends_with('{') {
                    continue;
                }

                if !trimmed.ends_with("\n\n") && !trimmed.ends_with("\r\n\r\n") {
                    diags.push(
                        Diagnostic::new(
                            rule_impl.name(),
                            "Expected empty line before comment".to_string(),
                        )
                        .severity(rule_impl.default_severity())
                        .span(Span::new(comment.span.offset, comment.span.length)),
                    );
                }
            }
        }

        // Recurse into at-rules
        if let CssNode::AtRule(at_rule) = node {
            check_comment_nodes(rule_impl, &at_rule.children, ctx, diags);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Comment, Span as ParserSpan, StyleRule, Syntax};

    fn make_ctx(source: &str) -> RuleContext {
        RuleContext {
            file_path: "t.css",
            source,
            syntax: Syntax::Css, options: None }
    }

    #[test]
    fn reports_missing_empty_line_before_comment() {
        let src = "a { color: red; }\n/* comment */";
        let comment_offset = src.find("/*").unwrap();
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 17),
            }),
            CssNode::Comment(Comment {
                is_line: false,
                text: "/* comment */".to_string(),
                span: ParserSpan::new(comment_offset, 13),
            }),
        ];
        let d = CommentEmptyLineBefore.check_root(&nodes, &make_ctx(src));
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("comment"));
    }

    #[test]
    fn allows_empty_line_before_comment() {
        let src = "a { color: red; }\n\n/* comment */";
        let comment_offset = src.find("/*").unwrap();
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 17),
            }),
            CssNode::Comment(Comment {
                is_line: false,
                text: "/* comment */".to_string(),
                span: ParserSpan::new(comment_offset, 13),
            }),
        ];
        let d = CommentEmptyLineBefore.check_root(&nodes, &make_ctx(src));
        assert!(d.is_empty());
    }

    #[test]
    fn skips_scss_double_slash_comments() {
        let src = "a { color: red; }\n// scss comment";
        let comment_offset = src.find("//").unwrap();
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 17),
            }),
            CssNode::Comment(Comment {
                is_line: true,
                text: " scss comment".to_string(),
                span: ParserSpan::new(comment_offset, 15),
            }),
        ];
        let ctx = RuleContext {
            file_path: "t.scss",
            source: src,
            syntax: Syntax::Scss, options: None };
        let d = CommentEmptyLineBefore.check_root(&nodes, &ctx);
        assert!(d.is_empty());
    }

    #[test]
    fn allows_first_nested_comment() {
        let src = "/* first comment */";
        let nodes = vec![CssNode::Comment(Comment {
            is_line: false,
            text: "/* first comment */".to_string(),
            span: ParserSpan::new(0, src.len()),
        })];
        let d = CommentEmptyLineBefore.check_root(&nodes, &make_ctx(src));
        assert!(d.is_empty());
    }
}
