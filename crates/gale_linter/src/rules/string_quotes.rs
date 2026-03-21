use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Edit, Fix, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Enforce double quotes around string values.
///
/// Equivalent to Stylelint's `string-quotes` rule with "double" option.
pub struct StringQuotes;

impl Rule for StringQuotes {
    fn name(&self) -> &'static str {
        "string-quotes"
    }

    fn description(&self) -> &'static str {
        "Enforce double quotes around strings"
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
            let source_slice = if decl_end <= ctx.source.len() && decl_start < decl_end {
                &ctx.source[decl_start..decl_end]
            } else {
                &decl.value
            };
            check_single_quotes(
                self,
                source_slice,
                decl_start,
                decl_end <= ctx.source.len(),
                &mut diags,
            );
        }
        diags
    }
}

fn check_single_quotes(
    rule: &StringQuotes,
    text: &str,
    base_offset: usize,
    use_base: bool,
    diags: &mut Vec<Diagnostic>,
) {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'\'' {
            let string_start = i;
            i += 1;
            let mut contents = String::new();
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    contents.push('\\');
                    contents.push(bytes[i + 1] as char);
                    i += 2;
                    continue;
                }
                if bytes[i] == b'\'' {
                    i += 1;
                    break;
                }
                contents.push(bytes[i] as char);
                i += 1;
            }
            // Escape any double quotes inside the string
            let escaped = contents.replace('"', "\\\"");
            let replacement = format!("\"{escaped}\"");
            let abs_offset = if use_base {
                base_offset + string_start
            } else {
                base_offset
            };
            let span_len = i - string_start;
            diags.push(
                Diagnostic::new(rule.name(), "Expected double quotes")
                    .severity(rule.default_severity())
                    .span(Span::new(abs_offset, span_len))
                    .fix(Fix::new(
                        "Replace single quotes with double quotes",
                        vec![Edit::new(Span::new(abs_offset, span_len), &replacement)],
                    )),
            );
        } else if bytes[i] == b'"' {
            // Skip over double-quoted strings
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
        } else {
            i += 1;
        }
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
            options: None,
        }
    }

    fn style_with_value(val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "content".to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_single_quotes() {
        let d = StringQuotes.check(&style_with_value("'hello'"), &ctx());
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].message, "Expected double quotes");
        assert!(d[0].fix.is_some());
    }

    #[test]
    fn allows_double_quotes() {
        let d = StringQuotes.check(&style_with_value("\"hello\""), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn fixes_single_to_double_quotes() {
        let d = StringQuotes.check(&style_with_value("'hello'"), &ctx());
        assert_eq!(d.len(), 1);
        let fix = d[0].fix.as_ref().unwrap();
        assert_eq!(fix.edits[0].new_text, "\"hello\"");
    }
}
