use gale_css_parser::{CssNode, Syntax};
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow quoted strings inside `quote()`.
pub struct ScssFunctionQuoteNoQuotedStringsInside;

impl Rule for ScssFunctionQuoteNoQuotedStringsInside {
    fn name(&self) -> &'static str {
        "scss/function-quote-no-quoted-strings-inside"
    }

    fn description(&self) -> &'static str {
        "Disallow quoted strings inside quote()"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        if !matches!(ctx.syntax, Syntax::Scss | Syntax::Sass) {
            return vec![];
        }

        let CssNode::Declaration(decl) = node else {
            return vec![];
        };

        let value = &decl.value;
        // Simple pattern: look for `quote("` or `quote('`
        if let Some(idx) = value.find("quote(") {
            let after = &value[idx + 6..];
            let trimmed = after.trim_start();
            if trimmed.starts_with('"') || trimmed.starts_with('\'') {
                return vec![
                    Diagnostic::new(self.name(), "Unexpected quoted string inside quote()")
                        .severity(self.default_severity())
                        .span(Span::new(decl.span.offset, decl.span.length)),
                ];
            }
        }

        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, Syntax};

    fn scss_ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "t.scss",
            source: "",
            syntax: Syntax::Scss,
            options: None,
        }
    }

    fn decl(value: &str) -> CssNode {
        CssNode::Declaration(Declaration {
            property: "content".to_string(),
            value: value.to_string(),
            span: ParserSpan::new(0, 10),
            important: false,
        })
    }

    #[test]
    fn reports_quoted_string_inside_quote() {
        let d =
            ScssFunctionQuoteNoQuotedStringsInside.check(&decl("quote(\"hello\")"), &scss_ctx());
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn allows_unquoted_inside_quote() {
        let d = ScssFunctionQuoteNoQuotedStringsInside.check(&decl("quote($var)"), &scss_ctx());
        assert!(d.is_empty());
    }
}
