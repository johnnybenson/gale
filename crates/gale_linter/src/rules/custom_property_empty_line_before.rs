use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require an empty line before custom properties (`--*`), except first-nested
/// or after another custom property.
///
/// Equivalent to Stylelint's `custom-property-empty-line-before` rule with "always" option.
/// Detection-only (no autofix).
pub struct CustomPropertyEmptyLineBefore;

impl Rule for CustomPropertyEmptyLineBefore {
    fn name(&self) -> &'static str {
        "custom-property-empty-line-before"
    }

    fn description(&self) -> &'static str {
        "Require an empty line before custom properties"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let mut diags = Vec::new();

        for (i, decl) in rule.declarations.iter().enumerate() {
            // Only check custom properties (starting with --)
            if !decl.property.starts_with("--") {
                continue;
            }

            // Exception: first declaration in block
            if i == 0 {
                continue;
            }

            // Exception: previous declaration is also a custom property
            if rule.declarations[i - 1].property.starts_with("--") {
                continue;
            }

            let decl_start = decl.span.offset;
            if decl_start == 0 || decl_start > ctx.source.len() {
                continue;
            }

            let before = &ctx.source[..decl_start];
            let trimmed = before.trim_end_matches([' ', '\t']);
            if !trimmed.ends_with("\n\n") && !trimmed.ends_with("\r\n\r\n") {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected empty line before custom property \"{}\"",
                            decl.property
                        ),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(decl.span.offset, decl.span.length)),
                );
            }
        }
        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn make_ctx(source: &str) -> RuleContext {
        RuleContext {
            file_path: "t.css",
            source,
            syntax: Syntax::Css,
            options: None,
        }
    }

    #[test]
    fn reports_missing_empty_line_before_custom_property() {
        let src = "a {\n  color: red;\n  --my-var: blue;\n}";
        let var_offset = src.find("--my-var").unwrap();
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(src.find("color").unwrap(), "color: red;".len()),
                    important: false,
                },
                Declaration {
                    property: "--my-var".to_string(),
                    value: "blue".to_string(),
                    span: ParserSpan::new(var_offset, "--my-var: blue;".len()),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, src.len()),
        });
        let d = CustomPropertyEmptyLineBefore.check(&node, &make_ctx(src));
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("--my-var"));
    }

    #[test]
    fn allows_empty_line_before_custom_property() {
        let src = "a {\n  color: red;\n\n  --my-var: blue;\n}";
        let var_offset = src.find("--my-var").unwrap();
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(src.find("color").unwrap(), "color: red;".len()),
                    important: false,
                },
                Declaration {
                    property: "--my-var".to_string(),
                    value: "blue".to_string(),
                    span: ParserSpan::new(var_offset, "--my-var: blue;".len()),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, src.len()),
        });
        let d = CustomPropertyEmptyLineBefore.check(&node, &make_ctx(src));
        assert!(d.is_empty());
    }

    #[test]
    fn allows_first_custom_property_in_block() {
        let src = "a {\n  --my-var: blue;\n}";
        let var_offset = src.find("--my-var").unwrap();
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "--my-var".to_string(),
                value: "blue".to_string(),
                span: ParserSpan::new(var_offset, "--my-var: blue;".len()),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, src.len()),
        });
        let d = CustomPropertyEmptyLineBefore.check(&node, &make_ctx(src));
        assert!(d.is_empty());
    }

    #[test]
    fn allows_consecutive_custom_properties() {
        let src = "a {\n  color: red;\n\n  --a: 1;\n  --b: 2;\n}";
        let a_offset = src.find("--a").unwrap();
        let b_offset = src.find("--b").unwrap();
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(src.find("color").unwrap(), "color: red;".len()),
                    important: false,
                },
                Declaration {
                    property: "--a".to_string(),
                    value: "1".to_string(),
                    span: ParserSpan::new(a_offset, "--a: 1;".len()),
                    important: false,
                },
                Declaration {
                    property: "--b".to_string(),
                    value: "2".to_string(),
                    span: ParserSpan::new(b_offset, "--b: 2;".len()),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, src.len()),
        });
        let d = CustomPropertyEmptyLineBefore.check(&node, &make_ctx(src));
        assert!(d.is_empty());
    }
}
