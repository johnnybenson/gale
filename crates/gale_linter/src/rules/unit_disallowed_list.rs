use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow specified units globally.
///
/// Options: an array of disallowed unit strings.
/// Example: `["px", "pt"]`
///
/// Equivalent to Stylelint's `unit-disallowed-list` rule.
pub struct UnitDisallowedList;

/// Extract all units from a CSS value string.
fn extract_units(value: &str) -> Vec<String> {
    let mut units = Vec::new();
    let bytes = value.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i].is_ascii_digit() || (bytes[i] == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit()) {
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let unit_start = i;
            while i < len && (bytes[i].is_ascii_alphabetic() || bytes[i] == b'%') {
                i += 1;
            }
            if i > unit_start {
                units.push(value[unit_start..i].to_string());
            }
        } else {
            i += 1;
        }
    }

    units
}

fn parse_disallowed_list(options: Option<&serde_json::Value>) -> Vec<String> {
    let Some(val) = options else {
        return Vec::new();
    };
    let Some(arr) = val.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|v| v.as_str().map(|s| s.to_ascii_lowercase()))
        .collect()
}

impl Rule for UnitDisallowedList {
    fn name(&self) -> &'static str {
        "unit-disallowed-list"
    }

    fn description(&self) -> &'static str {
        "Specify a list of disallowed units"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let disallowed = parse_disallowed_list(ctx.options);
        if disallowed.is_empty() {
            return vec![];
        }

        let mut diags = Vec::new();
        let declarations: Vec<&gale_css_parser::Declaration> = match node {
            CssNode::Style(rule) => rule.declarations.iter().collect(),
            CssNode::Declaration(decl) => vec![decl],
            _ => return vec![],
        };

        for decl in declarations {
            let units = extract_units(&decl.value);
            for unit in units {
                let unit_lower = unit.to_ascii_lowercase();
                if disallowed.contains(&unit_lower) {
                    diags.push(
                        Diagnostic::new(
                            self.name(),
                            format!("Unexpected unit \"{unit}\""),
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

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};
    use serde_json::json;

    fn ctx_with_options(opts: &serde_json::Value) -> RuleContext<'_> {
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: Some(opts),
        }
    }

    fn ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        }
    }

    fn style_with_decl(prop: &str, val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: prop.to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, 10),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn allows_all_when_no_options() {
        let d = UnitDisallowedList.check(&style_with_decl("margin", "10px"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_non_disallowed_unit() {
        let opts = json!(["pt"]);
        let d = UnitDisallowedList.check(
            &style_with_decl("margin", "10px"),
            &ctx_with_options(&opts),
        );
        assert!(d.is_empty());
    }

    #[test]
    fn rejects_disallowed_unit() {
        let opts = json!(["px"]);
        let d = UnitDisallowedList.check(
            &style_with_decl("margin", "10px"),
            &ctx_with_options(&opts),
        );
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("px"));
    }

    #[test]
    fn rejects_multiple_disallowed_units() {
        let opts = json!(["px", "em"]);
        let d = UnitDisallowedList.check(
            &style_with_decl("margin", "10px 1.5em"),
            &ctx_with_options(&opts),
        );
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn case_insensitive_unit_match() {
        let opts = json!(["PX"]);
        let d = UnitDisallowedList.check(
            &style_with_decl("margin", "10px"),
            &ctx_with_options(&opts),
        );
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn rule_name_is_correct() {
        assert_eq!(UnitDisallowedList.name(), "unit-disallowed-list");
    }
}
