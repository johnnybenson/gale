use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Edit, Fix, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Enforces lowercase function names in values.
///
/// Equivalent to Stylelint's `function-name-case` rule.
pub struct FunctionNameCase;

/// CSS functions whose canonical name contains uppercase letters.
/// These should not be flagged by the lowercase enforcement rule.
const CAMEL_CASE_FUNCTIONS: &[&str] = &[
    "translateX",
    "translateY",
    "translateZ",
    "translate3d",
    "scaleX",
    "scaleY",
    "scaleZ",
    "scale3d",
    "rotateX",
    "rotateY",
    "rotateZ",
    "rotate3d",
    "skewX",
    "skewY",
    "perspective",
];

/// Returns true if the function name is a known camelCase CSS function
/// (case-insensitive match against the canonical name).
fn is_camel_case_css_function(name: &str) -> bool {
    CAMEL_CASE_FUNCTIONS
        .iter()
        .any(|&f| f.eq_ignore_ascii_case(name))
}

impl Rule for FunctionNameCase {
    fn name(&self) -> &'static str {
        "function-name-case"
    }

    fn description(&self) -> &'static str {
        "Specify lowercase for function names"
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
            let has_source = decl_end <= ctx.source.len() && decl_start < decl_end;

            for func_name in extract_function_names(&decl.value) {
                if func_name != func_name.to_ascii_lowercase()
                    && !is_camel_case_css_function(&func_name)
                {
                    let lowered = func_name.to_ascii_lowercase();

                    // Try to find the function name in the source to build a fix
                    let fix = if has_source {
                        let search_area = &ctx.source[decl_start..decl_end];
                        // Find the exact function name followed by '('
                        let target = format!("{func_name}(");
                        search_area.find(&target).map(|rel_offset| {
                            let abs_offset = decl_start + rel_offset;
                            Fix::new(
                                format!("Lowercase function name to \"{lowered}\""),
                                vec![Edit::new(
                                    Span::new(abs_offset, func_name.len()),
                                    &lowered,
                                )],
                            )
                        })
                    } else {
                        None
                    };

                    let mut diag = Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected \"{func_name}\" to be \"{lowered}\"",
                        ),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(decl.span.offset, decl.span.length));

                    if let Some(f) = fix {
                        diag = diag.fix(f);
                    }

                    diags.push(diag);
                }
            }
        }
        diags
    }
}

fn extract_function_names(value: &str) -> Vec<String> {
    let mut names = Vec::new();
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Look for a word followed by '('
        if chars[i].is_ascii_alphabetic() || chars[i] == '-' || chars[i] == '_' {
            let start = i;
            while i < len
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '-' || chars[i] == '_')
            {
                i += 1;
            }
            if i < len && chars[i] == '(' {
                let name: String = chars[start..i].iter().collect();
                names.push(name);
            }
        } else {
            i += 1;
        }
    }

    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css }
    }

    fn style_decl(val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "color".to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_uppercase_function() {
        let d = FunctionNameCase.check(&style_decl("RGB(0, 0, 0)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("rgb"));
    }

    #[test]
    fn allows_lowercase_function() {
        assert!(FunctionNameCase.check(&style_decl("rgb(0, 0, 0)"), &ctx()).is_empty());
    }

    #[test]
    fn reports_mixed_case_function() {
        let d = FunctionNameCase.check(&style_decl("Calc(100% - 20px)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("calc"));
    }

    #[test]
    fn emits_fix_for_uppercase_function() {
        let source = "a { color: RGB(0, 0, 0); }";
        let ctx = RuleContext { file_path: "t.css", source, syntax: Syntax::Css };
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "color".to_string(),
                value: "RGB(0, 0, 0)".to_string(),
                span: ParserSpan::new(4, 20),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, source.len()),
        });
        let d = FunctionNameCase.check(&node, &ctx);
        assert_eq!(d.len(), 1);
        assert!(d[0].fix.is_some());
        let fix = d[0].fix.as_ref().unwrap();
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].new_text, "rgb");
    }
}
