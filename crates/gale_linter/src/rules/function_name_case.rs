use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Enforces lowercase function names in values.
///
/// Equivalent to Stylelint's `function-name-case` rule.
pub struct FunctionNameCase;

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

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let mut diags = Vec::new();
        for decl in &rule.declarations {
            for func_name in extract_function_names(&decl.value) {
                if func_name != func_name.to_ascii_lowercase() {
                    diags.push(
                        Diagnostic::new(
                            self.name(),
                            format!(
                                "Expected \"{func_name}\" to be \"{}\"",
                                func_name.to_ascii_lowercase()
                            ),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(decl.span.offset, decl.span.length)),
                    );
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
}
