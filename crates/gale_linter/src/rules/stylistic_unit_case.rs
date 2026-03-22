use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Edit, Fix, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Specify lowercase or uppercase for units.
///
/// Equivalent to `@stylistic/unit-case`.
pub struct StylisticUnitCase;

impl Rule for StylisticUnitCase {
    fn name(&self) -> &'static str {
        "@stylistic/unit-case"
    }

    fn description(&self) -> &'static str {
        "Specify lowercase or uppercase for units"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let expected_case = ctx.primary_option_str().unwrap_or("lower");

        let decls: Vec<_> = match node {
            CssNode::Style(rule) => rule.declarations.iter().collect(),
            CssNode::Declaration(decl) => vec![decl],
            _ => return vec![],
        };

        let mut diags = Vec::new();
        for decl in decls {
            let value = &decl.value;
            let base = decl.span.offset;
            // Find units in value: a number followed by letters
            for (unit_offset, unit) in find_units(value) {
                let is_wrong = match expected_case {
                    "lower" => unit.chars().any(|c| c.is_ascii_uppercase()),
                    "upper" => unit.chars().any(|c| c.is_ascii_lowercase()),
                    _ => false,
                };
                if is_wrong {
                    let fixed = match expected_case {
                        "lower" => unit.to_ascii_lowercase(),
                        "upper" => unit.to_ascii_uppercase(),
                        _ => continue,
                    };
                    // Try to find the unit in source for accurate offset
                    let abs_offset = base + unit_offset;
                    diags.push(
                        Diagnostic::new(
                            self.name(),
                            format!("Expected \"{unit}\" to be \"{fixed}\""),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(abs_offset, unit.len()))
                        .fix(Fix::new(
                            format!("Convert to {expected_case}case"),
                            vec![Edit::new(Span::new(abs_offset, unit.len()), &fixed)],
                        )),
                    );
                }
            }
        }
        diags
    }
}

/// Find units in a CSS value string. Returns (offset, unit_str) pairs.
fn find_units(value: &str) -> Vec<(usize, String)> {
    let mut units = Vec::new();
    let bytes = value.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Skip past a number (digits, dots)
        if bytes[i].is_ascii_digit()
            || (bytes[i] == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit())
        {
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            // Now check for a unit (alphabetic chars or %)
            if i < len && (bytes[i].is_ascii_alphabetic() || bytes[i] == b'%') {
                let unit_start = i;
                while i < len && bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                if i > unit_start {
                    let unit = &value[unit_start..i];
                    units.push((unit_start, unit.to_string()));
                }
            }
        } else {
            i += 1;
        }
    }
    units
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
                property: "width".to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, val.len()),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_uppercase_unit() {
        let rule = StylisticUnitCase;
        let d = rule.check(&style_with_value("10PX"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("\"px\""));
    }

    #[test]
    fn allows_lowercase_unit() {
        let rule = StylisticUnitCase;
        let d = rule.check(&style_with_value("10px"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn reports_mixed_case_unit() {
        let rule = StylisticUnitCase;
        let d = rule.check(&style_with_value("10Rem"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("\"rem\""));
    }
}
