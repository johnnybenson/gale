use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports empty CSS comments (`/* */` or `/**/`).
///
/// Equivalent to Stylelint's `comment-no-empty` rule.
pub struct CommentNoEmpty;

impl Rule for CommentNoEmpty {
    fn name(&self) -> &'static str {
        "comment-no-empty"
    }

    fn description(&self) -> &'static str {
        "Disallow empty comments"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _context: &RuleContext) -> Vec<Diagnostic> {
        match node {
            CssNode::Comment(comment) => {
                // The text field contains the full comment including delimiters.
                // Strip `/*` and `*/` and check if the content is empty/whitespace.
                let inner = comment
                    .text
                    .trim_start_matches("/*")
                    .trim_end_matches("*/");
                if inner.trim().is_empty() {
                    vec![Diagnostic::new(self.name(), "Unexpected empty comment")
                        .severity(self.default_severity())
                        .span(Span::new(comment.span.offset, comment.span.length))]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Comment, Span as ParserSpan, Syntax};

    fn make_context() -> RuleContext<'static> {
        RuleContext {
            file_path: "test.css",
            source: "",
            syntax: Syntax::Css,
        }
    }

    #[test]
    fn reports_empty_comment() {
        let rule = CommentNoEmpty;
        let node = CssNode::Comment(Comment {
            text: "/* */".to_string(),
            span: ParserSpan::new(0, 5),
        });
        let diags = rule.check(&node, &make_context());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "Unexpected empty comment");
    }

    #[test]
    fn reports_minimal_empty_comment() {
        let rule = CommentNoEmpty;
        let node = CssNode::Comment(Comment {
            text: "/**/".to_string(),
            span: ParserSpan::new(0, 4),
        });
        let diags = rule.check(&node, &make_context());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn ignores_non_empty_comment() {
        let rule = CommentNoEmpty;
        let node = CssNode::Comment(Comment {
            text: "/* hello */".to_string(),
            span: ParserSpan::new(0, 11),
        });
        let diags = rule.check(&node, &make_context());
        assert!(diags.is_empty());
    }
}
