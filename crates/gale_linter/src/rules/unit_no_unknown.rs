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
///
/// Skips:
/// - Hex colors (`#fff`)
/// - Content inside `var(…)` (custom property references may contain
///   number-letter sequences like `--spacing-2xl` that aren't units)
/// - Custom property names (starting with `--`)
/// - SCSS variables (`$var-2x`)
fn extract_units(value: &str) -> Vec<String> {
    let mut units = Vec::new();
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Skip quoted strings — content inside quotes is not CSS values
        // and should not be parsed for units (e.g. WCAG level "AA").
        if chars[i] == '"' || chars[i] == '\'' {
            let quote = chars[i];
            i += 1;
            while i < len && chars[i] != quote {
                if chars[i] == '\\' {
                    i += 1; // skip escaped char
                }
                i += 1;
            }
            if i < len {
                i += 1; // skip closing quote
            }
            continue;
        }

        // Skip content inside var(…) — custom property names can contain
        // number+letter sequences that aren't actual CSS units.
        // Also skip url(…) — data URLs contain embedded content.
        if i + 3 < len
            && ((chars[i] == 'v'
                && chars[i + 1] == 'a'
                && chars[i + 2] == 'r'
                && chars[i + 3] == '(')
                || (chars[i] == 'u'
                    && chars[i + 1] == 'r'
                    && chars[i + 2] == 'l'
                    && chars[i + 3] == '('))
        {
            i += 4;
            let mut depth = 1;
            while i < len && depth > 0 {
                match chars[i] {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            continue;
        }

        // Skip custom property names (--name-2xl etc.)
        if i + 1 < len && chars[i] == '-' && chars[i + 1] == '-' {
            i += 2;
            while i < len
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '-' || chars[i] == '_')
            {
                i += 1;
            }
            continue;
        }

        // Skip SCSS variables ($var-name)
        if chars[i] == '$' {
            i += 1;
            while i < len
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '-' || chars[i] == '_')
            {
                i += 1;
            }
            continue;
        }

        // Skip hex colors (#fff, #ff00ff, etc.)
        if chars[i] == '#' {
            i += 1;
            while i < len && chars[i].is_ascii_hexdigit() {
                i += 1;
            }
            continue;
        }

        // Skip to a digit or decimal point followed by digit
        if chars[i].is_ascii_digit()
            || (chars[i] == '.' && i + 1 < len && chars[i + 1].is_ascii_digit())
        {
            // If the digit is preceded by `-` which is preceded by an
            // alphabetic char, this is part of a hyphenated identifier
            // (e.g. `preserve-3d`), not a number with a unit.
            if i > 1
                && chars[i - 1] == '-'
                && (chars[i - 2].is_ascii_alphanumeric()
                    || chars[i - 2] == '-'
                    || chars[i - 2] == '_')
            {
                // Skip the rest of this identifier
                while i < len
                    && (chars[i].is_ascii_alphanumeric() || chars[i] == '-' || chars[i] == '_')
                {
                    i += 1;
                }
                continue;
            }
            // Skip the number
            while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            // Now extract the unit (alphabetic chars or %)
            if i < len && (chars[i].is_ascii_alphabetic() || chars[i] == '%') {
                // But first, check if the char after the unit is `-` or `_`
                // which would mean this is an identifier, not a unit.
                let start = i;
                if chars[i] == '%' {
                    i += 1;
                } else {
                    while i < len && chars[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                }
                // If the next char is `-`, `_`, alphanumeric, or `(`, this is part
                // of an identifier or function name (e.g. `scale3d(`), not a unit.
                if i < len
                    && (chars[i] == '-'
                        || chars[i] == '_'
                        || chars[i] == '('
                        || chars[i].is_ascii_alphanumeric())
                {
                    // Skip rest of identifier
                    while i < len
                        && (chars[i].is_ascii_alphanumeric() || chars[i] == '-' || chars[i] == '_')
                    {
                        i += 1;
                    }
                    continue;
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
        assert!(
            UnitNoUnknown
                .check(&style_with_value("10px"), &ctx())
                .is_empty()
        );
        assert!(
            UnitNoUnknown
                .check(&style_with_value("2rem"), &ctx())
                .is_empty()
        );
        assert!(
            UnitNoUnknown
                .check(&style_with_value("50%"), &ctx())
                .is_empty()
        );
    }

    #[test]
    fn extract_units_from_complex_value() {
        let units = extract_units("calc(100% - 20px)");
        assert!(units.contains(&"%".to_string()));
        assert!(units.contains(&"px".to_string()));
    }

    #[test]
    fn does_not_extract_unit_from_function_names() {
        // scale3d, rotate3d, translate3d — the `3` followed by `d(` is a
        // function name, not a number with a unit.
        let units = extract_units("scale3d(1, 1, 1)");
        assert!(
            units.is_empty(),
            "Expected no units from scale3d, got: {:?}",
            units
        );
        let units = extract_units("rotate3d(0, 0, 1, 45deg)");
        assert!(
            units.contains(&"deg".to_string()),
            "Expected deg unit from rotate3d args"
        );
        assert!(
            !units.iter().any(|u| u == "d"),
            "Should not extract 'd' as unit from rotate3d"
        );
    }

    #[test]
    fn does_not_report_scale3d_as_unknown_unit() {
        let d = UnitNoUnknown.check(&style_with_value("scale3d(1.1, 1.1, 1.1)"), &ctx());
        assert!(d.is_empty(), "Expected no diagnostics, got: {:?}", d);
    }
}
