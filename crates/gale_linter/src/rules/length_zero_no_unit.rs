use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports units on zero lengths (e.g. `0px` → `0`).
///
/// Equivalent to Stylelint's `length-zero-no-unit` rule.
pub struct LengthZeroNoUnit;

const LENGTH_UNITS: &[&str] = &[
    "px", "em", "rem", "ex", "ch", "vw", "vh", "vmin", "vmax", "cm", "mm", "in", "pt", "pc",
    "q", "cap", "ic", "rlh", "lh", "vi", "vb", "cqw", "cqh", "cqi", "cqb", "cqmin", "cqmax",
    "dvw", "dvh", "lvw", "lvh", "svw", "svh",
];

impl Rule for LengthZeroNoUnit {
    fn name(&self) -> &'static str {
        "length-zero-no-unit"
    }

    fn description(&self) -> &'static str {
        "Disallow units for zero lengths"
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
            // Skip custom properties
            if decl.property.starts_with("--") {
                continue;
            }
            if has_zero_with_unit(&decl.value) {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Unexpected unit on zero length in \"{}\"",
                            decl.value
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

fn has_zero_with_unit(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Find a '0' that is a standalone number (not part of a larger number like 10px)
        if chars[i] == '0' {
            // Check it's not preceded by a digit or dot (i.e., it's the whole number)
            let is_start_of_number = i == 0
                || (!chars[i - 1].is_ascii_digit() && chars[i - 1] != '.');

            if is_start_of_number {
                // Check there's no dot or digit right after (0.5px is fine)
                let after = i + 1;
                if after < len && (chars[after] == '.' || chars[after].is_ascii_digit()) {
                    i += 1;
                    continue;
                }
                // Check if followed by a length unit
                let rest: String = chars[after..].iter().collect();
                for unit in LENGTH_UNITS {
                    if rest.starts_with(unit) {
                        // Make sure the unit isn't part of a longer word
                        let end = after + unit.len();
                        if end >= len || !chars[end].is_ascii_alphabetic() {
                            return true;
                        }
                    }
                }
            }
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css }
    }

    fn style_decl(prop: &str, val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: prop.to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_zero_with_unit() {
        let d = LengthZeroNoUnit.check(&style_decl("margin", "0px"), &ctx());
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn allows_zero_without_unit() {
        assert!(LengthZeroNoUnit.check(&style_decl("margin", "0"), &ctx()).is_empty());
    }

    #[test]
    fn allows_non_zero_with_unit() {
        assert!(LengthZeroNoUnit.check(&style_decl("margin", "10px"), &ctx()).is_empty());
    }

    #[test]
    fn skips_custom_properties() {
        assert!(LengthZeroNoUnit.check(&style_decl("--my-var", "0px"), &ctx()).is_empty());
    }
}
