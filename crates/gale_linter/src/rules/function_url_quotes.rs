use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Edit, Fix, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require quotes around `url()` values.
///
/// Equivalent to Stylelint's `function-url-quotes` rule with "always" option.
pub struct FunctionUrlQuotes;

impl Rule for FunctionUrlQuotes {
    fn name(&self) -> &'static str {
        "function-url-quotes"
    }

    fn description(&self) -> &'static str {
        "Require quotes around url() values"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let mut diags = Vec::new();
        for decl in &rule.declarations {
            let decl_start = decl.span.offset;
            let decl_end = decl_start + decl.span.length;
            let search_area = if decl_end <= ctx.source.len() && decl_start < decl_end {
                &ctx.source[decl_start..decl_end]
            } else {
                &decl.value
            };

            for (rel_offset, url_content) in find_unquoted_urls(search_area) {
                let abs_offset = if decl_end <= ctx.source.len() && decl_start < decl_end {
                    decl_start + rel_offset
                } else {
                    decl_start
                };
                let quoted = format!("\"{}\"", url_content);
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!("Expected quotes around url() value \"{url_content}\""),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(abs_offset, url_content.len()))
                    .fix(Fix::new(
                        "Wrap URL in double quotes",
                        vec![Edit::new(
                            Span::new(abs_offset, url_content.len()),
                            &quoted,
                        )],
                    )),
                );
            }
        }
        diags
    }
}

/// Find unquoted URL contents inside `url(...)` calls.
/// Returns (byte_offset_of_content, content_string).
fn find_unquoted_urls(value: &str) -> Vec<(usize, String)> {
    let mut results = Vec::new();
    let lower = value.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(pos) = lower[search_from..].find("url(") {
        let abs_pos = search_from + pos;
        let content_start = abs_pos + 4; // skip "url("
        if content_start < value.len() {
            let rest = &value[content_start..];
            let first_non_ws = rest.bytes().position(|b| b != b' ' && b != b'\t');
            if let Some(fns) = first_non_ws {
                let first_char = rest.as_bytes()[fns];
                if first_char != b'"' && first_char != b'\'' {
                    // Find the closing paren
                    if let Some(close) = rest.find(')') {
                        let content = rest[..close].trim();
                        if !content.is_empty() {
                            let content_abs = content_start + fns;
                            results.push((content_abs, content.to_string()));
                        }
                    }
                }
            }
        }
        search_from = content_start;
    }
    results
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

    fn style_with_value(val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "background".to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_unquoted_url() {
        let d = FunctionUrlQuotes.check(&style_with_value("url(foo.png)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("foo.png"));
        assert!(d[0].fix.is_some());
    }

    #[test]
    fn allows_quoted_url() {
        let d = FunctionUrlQuotes.check(&style_with_value("url(\"foo.png\")"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_single_quoted_url() {
        let d = FunctionUrlQuotes.check(&style_with_value("url('foo.png')"), &ctx());
        assert!(d.is_empty());
    }
}
