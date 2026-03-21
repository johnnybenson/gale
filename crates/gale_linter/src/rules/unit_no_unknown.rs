use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::data::is_known_unit;
use crate::rule::{Rule, RuleContext};

pub struct UnitNoUnknown;

impl Rule for UnitNoUnknown {
    fn name(&self) -> &'static str {
        "unit-no-unknown"
    }

    fn description(&self) -> &'static str {
        "Disallow unknown units"
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
            // Skip `content` property — its values use CSS escapes (e.g. "\00A0")
            // that can be misinterpreted as units.
            if decl.property.eq_ignore_ascii_case("content") {
                continue;
            }
            for unit in extract_units(&decl.value) {
                if !is_known_unit(&unit) {
                    diags.push(
                        Diagnostic::new(self.name(), format!("Unexpected unknown unit \"{unit}\""))
                            .severity(self.default_severity())
                            .span(Span::new(decl.span.offset, decl.span.length)),
                    );
                }
            }
        }
        diags
    }
}

/// Extract units from a CSS value string.
/// Finds patterns like `10px`, `2.5em`, `.5rem`, etc.
fn extract_units(value: &str) -> Vec<String> {
    let mut units = Vec::new();
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Skip hex colors (#fff, #ff00ff, etc.)
        if chars[i] == '#' {
            i += 1;
            while i < len && chars[i].is_ascii_hexdigit() {
                i += 1;
            }
            continue;
        }
        // Skip to a digit or decimal point followed by digit
        if chars[i].is_ascii_digit() || (chars[i] == '.' && i + 1 < len && chars[i + 1].is_ascii_digit()) {
            // Skip the number
            while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            // Now extract the unit (alphabetic chars or %)
            if i < len && (chars[i].is_ascii_alphabetic() || chars[i] == '%') {
                let start = i;
                if chars[i] == '%' {
                    i += 1;
                } else {
                    while i < len && chars[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                }
                let unit: String = chars[start..i].iter().collect();
                units.push(unit);
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
    use gale_css_parser::{CssNode, Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css }
    }

    fn style_with_value(val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "width".to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_unknown_unit() {
        let d = UnitNoUnknown.check(&style_with_value("10xyz"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("xyz"));
    }

    #[test]
    fn allows_known_units() {
        assert!(UnitNoUnknown.check(&style_with_value("10px"), &ctx()).is_empty());
        assert!(UnitNoUnknown.check(&style_with_value("2rem"), &ctx()).is_empty());
        assert!(UnitNoUnknown.check(&style_with_value("50%"), &ctx()).is_empty());
    }

    #[test]
    fn extract_units_from_complex_value() {
        let units = extract_units("calc(100% - 20px)");
        assert!(units.contains(&"%".to_string()));
        assert!(units.contains(&"px".to_string()));
    }
}
