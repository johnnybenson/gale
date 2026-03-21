use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Only allow specified CSS functions.
///
/// Options: an array of function names that are allowed. All other functions
/// are flagged.
///
/// Equivalent to Stylelint's `function-allowed-list` rule.
pub struct FunctionAllowedList;

impl Rule for FunctionAllowedList {
    fn name(&self) -> &'static str {
        "function-allowed-list"
    }

    fn description(&self) -> &'static str {
        "Specify a list of allowed functions"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let allowed: Vec<String> = match ctx.options {
            Some(serde_json::Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_ascii_lowercase()))
                .collect(),
            _ => return vec![],
        };

        let mut diags = Vec::new();

        match node {
            CssNode::Style(rule) => {
                for decl in &rule.declarations {
                    find_disallowed_functions(
                        &decl.value,
                        decl.span.offset,
                        &allowed,
                        self,
                        &mut diags,
                    );
                }
            }
            CssNode::Declaration(decl) => {
                find_disallowed_functions(
                    &decl.value,
                    decl.span.offset,
                    &allowed,
                    self,
                    &mut diags,
                );
            }
            _ => {}
        }

        diags
    }
}

fn find_disallowed_functions(
    value: &str,
    base_offset: usize,
    allowed: &[String],
    rule: &FunctionAllowedList,
    diags: &mut Vec<Diagnostic>,
) {
    let lower = value.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'(' && i > 0 {
            let end = i;
            let mut start = i;
            while start > 0
                && (bytes[start - 1].is_ascii_alphanumeric()
                    || bytes[start - 1] == b'-'
                    || bytes[start - 1] == b'_')
            {
                start -= 1;
            }
            if start < end {
                let fname = &lower[start..end];
                if !allowed.contains(&fname.to_string()) {
                    diags.push(
                        Diagnostic::new(rule.name(), format!("Unexpected function \"{fname}\""))
                            .severity(rule.default_severity())
                            .span(Span::new(base_offset + start, end - start)),
                    );
                }
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx_with_options(options: Option<serde_json::Value>) -> RuleContext<'static> {
        let opts: Option<&'static serde_json::Value> = options.map(|v| &*Box::leak(Box::new(v)));
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: opts,
        }
    }

    fn style_with_value(val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "color".to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, val.len()),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn allows_all_when_no_options() {
        let ctx = ctx_with_options(None);
        let d = FunctionAllowedList.check(&style_with_value("rgb(0, 0, 0)"), &ctx);
        assert!(d.is_empty());
    }

    #[test]
    fn allows_listed_function() {
        let ctx = ctx_with_options(Some(serde_json::json!(["rgb", "hsl"])));
        let d = FunctionAllowedList.check(&style_with_value("rgb(0, 0, 0)"), &ctx);
        assert!(d.is_empty());
    }

    #[test]
    fn flags_unlisted_function() {
        let ctx = ctx_with_options(Some(serde_json::json!(["rgb"])));
        let d = FunctionAllowedList.check(&style_with_value("hsl(0, 100%, 50%)"), &ctx);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("hsl"));
    }

    #[test]
    fn allows_non_function_values() {
        let ctx = ctx_with_options(Some(serde_json::json!(["rgb"])));
        let d = FunctionAllowedList.check(&style_with_value("red"), &ctx);
        assert!(d.is_empty());
    }

    #[test]
    fn case_insensitive() {
        let ctx = ctx_with_options(Some(serde_json::json!(["rgb"])));
        let d = FunctionAllowedList.check(&style_with_value("RGB(0, 0, 0)"), &ctx);
        assert!(d.is_empty());
    }

    #[test]
    fn rule_name_is_correct() {
        assert_eq!(FunctionAllowedList.name(), "function-allowed-list");
    }
}
