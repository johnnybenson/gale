use gale_css_parser::{CssNode, Syntax};
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require or disallow whitespace after `//` in SCSS line comments.
///
/// By default expects `"always"` — a space after `//`:
///
/// ```scss
/// // Good
/// // comment text
///
/// // Bad
/// //comment text
/// ```
///
/// Equivalent to `scss/double-slash-comment-whitespace-inside`.
pub struct ScssDoubleSlashCommentWhitespaceInside;

impl Rule for ScssDoubleSlashCommentWhitespaceInside {
    fn name(&self) -> &'static str {
        "scss/double-slash-comment-whitespace-inside"
    }

    fn description(&self) -> &'static str {
        "Require or disallow whitespace after // in line comments"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        if !matches!(ctx.syntax, Syntax::Scss | Syntax::Sass) {
            return vec![];
        }

        let CssNode::Comment(comment) = node else {
            return vec![];
        };

        // Only applies to line comments (`//`)
        if !comment.is_line {
            return vec![];
        }

        let option = ctx.primary_option_str().unwrap_or("always");

        // The comment text should be the content after `//`.
        // Some parsers include the `//` prefix, some don't.
        let content = if comment.text.starts_with("//") {
            &comment.text[2..]
        } else {
            &comment.text
        };

        // Empty comments (`//` with nothing after) are always allowed.
        if content.is_empty() {
            return vec![];
        }

        let starts_with_space = content.starts_with(' ') || content.starts_with('\t');

        match option {
            "always" => {
                if !starts_with_space {
                    vec![
                        Diagnostic::new(
                            self.name(),
                            "Expected whitespace after //".to_string(),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(comment.span.offset, comment.span.length)),
                    ]
                } else {
                    vec![]
                }
            }
            "never" => {
                if starts_with_space {
                    vec![
                        Diagnostic::new(
                            self.name(),
                            "Unexpected whitespace after //".to_string(),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(comment.span.offset, comment.span.length)),
                    ]
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

    fn scss_ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "t.scss",
            source: "",
            syntax: Syntax::Scss,
            options: None,
        }
    }

    fn line_comment(text: &str) -> CssNode {
        CssNode::Comment(Comment {
            is_line: true,
            text: text.to_string(),
            span: ParserSpan::new(0, text.len()),
        })
    }

    fn block_comment(text: &str) -> CssNode {
        CssNode::Comment(Comment {
            is_line: false,
            text: text.to_string(),
            span: ParserSpan::new(0, text.len()),
        })
    }

    #[test]
    fn reports_missing_space_always() {
        let d =
            ScssDoubleSlashCommentWhitespaceInside.check(&line_comment("//comment"), &scss_ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("Expected whitespace"));
    }

    #[test]
    fn allows_space_always() {
        let d =
            ScssDoubleSlashCommentWhitespaceInside.check(&line_comment("// comment"), &scss_ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_empty_comment() {
        let d = ScssDoubleSlashCommentWhitespaceInside.check(&line_comment("//"), &scss_ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn skips_block_comments() {
        let d = ScssDoubleSlashCommentWhitespaceInside
            .check(&block_comment("/*comment*/"), &scss_ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn skips_non_scss() {
        let css_ctx = RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        };
        assert!(
            ScssDoubleSlashCommentWhitespaceInside
                .check(&line_comment("//comment"), &css_ctx)
                .is_empty()
        );
    }
}
