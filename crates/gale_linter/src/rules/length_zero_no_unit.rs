use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Edit, Fix, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports units on zero lengths (e.g. `0px` → `0`).
///
/// Equivalent to Stylelint's `length-zero-no-unit` rule.
pub struct LengthZeroNoUnit;

const LENGTH_UNITS: &[&str] = &[
    "px", "em", "rem", "ex", "ch", "vw", "vh", "vmin", "vmax", "cm", "mm", "in", "pt", "pc", "q",
    "cap", "ic", "rlh", "lh", "vi", "vb", "cqw", "cqh", "cqi", "cqb", "cqmin", "cqmax", "dvw",
    "dvh", "lvw", "lvh", "svw", "svh",
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

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let mut diags = Vec::new();
        for decl in &rule.declarations {
            // Skip custom properties
            if decl.property.starts_with("--") {
                continue;
            }

            // Skip values containing SCSS module function calls (e.g.
            // `custom-property.get-var('offset', 0rem)`).  The `0unit`
            // inside a SCSS function argument may be intentional and
            // Stylelint's PostCSS SCSS parser does not evaluate these.
            if decl.value.contains("#{") || decl.value.contains("@{") {
                continue;
            }
            // Detect SCSS module function calls: `namespace.function(`
            if has_scss_module_function(&decl.value) {
                continue;
            }

            let decl_start = decl.span.offset;
            let decl_end = decl_start + decl.span.length;
            let search_area = if decl_end <= ctx.source.len() && decl_start < decl_end {
                &ctx.source[decl_start..decl_end]
            } else {
                &decl.value
            };

            for (rel_offset, zero_unit_len) in find_zero_with_units(search_area) {
                let abs_offset = if decl_end <= ctx.source.len() && decl_start < decl_end {
                    decl_start + rel_offset
                } else {
                    decl_start
                };
                // Stylelint points to the unit part (after the zero), not the
                // zero itself.  The zero is 1 byte, so the unit starts at +1.
                let unit_offset = abs_offset + 1;
                let unit_len = zero_unit_len - 1;
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        "Unexpected unit".to_string(),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(unit_offset, unit_len))
                    .fix(Fix::new(
                        "Remove unit",
                        vec![Edit::new(Span::new(abs_offset, zero_unit_len), "0")],
                    )),
                );
            }
        }
        diags
    }
}

/// Check if a value contains a SCSS module-style function call like
/// `namespace.function-name(...)` where the dot indicates a SCSS module call.
fn has_scss_module_function(value: &str) -> bool {
    let bytes = value.as_bytes();
    for i in 0..bytes.len().saturating_sub(1) {
        if bytes[i] == b'.'
            && i > 0
            && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'-' || bytes[i - 1] == b'_')
            && (bytes[i + 1].is_ascii_alphabetic() || bytes[i + 1] == b'-' || bytes[i + 1] == b'_')
        {
            return true;
        }
    }
    false
}

/// Find all `0<unit>` patterns and return (byte_offset, total_length_including_zero).
fn find_zero_with_units(value: &str) -> Vec<(usize, usize)> {
    let bytes = value.as_bytes();
    let len = bytes.len();
    let mut results = Vec::new();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'0' {
            // Check it's not preceded by a digit or dot
            let is_start = i == 0 || (!bytes[i - 1].is_ascii_digit() && bytes[i - 1] != b'.');

            if is_start {
                let after = i + 1;
                // Skip if followed by dot or digit (e.g. 0.5px)
                if after < len && (bytes[after] == b'.' || bytes[after].is_ascii_digit()) {
                    i += 1;
                    continue;
                }
                // Check if followed by a length unit
                let rest = &value[after..].to_ascii_lowercase();
                for unit in LENGTH_UNITS {
                    if rest.starts_with(unit) {
                        let end = after + unit.len();
                        if end >= len || !bytes[end].is_ascii_alphabetic() {
                            results.push((i, 1 + unit.len())); // "0" + unit
                            i = end;
                            break;
                        }
                    }
                }
            }
        }
        i += 1;
    }
    results
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
        assert!(d[0].fix.is_some());
    }

    #[test]
    fn allows_zero_without_unit() {
        assert!(
            LengthZeroNoUnit
                .check(&style_decl("margin", "0"), &ctx())
                .is_empty()
        );
    }

    #[test]
    fn allows_non_zero_with_unit() {
        assert!(
            LengthZeroNoUnit
                .check(&style_decl("margin", "10px"), &ctx())
                .is_empty()
        );
    }

    #[test]
    fn skips_custom_properties() {
        assert!(
            LengthZeroNoUnit
                .check(&style_decl("--my-var", "0px"), &ctx())
                .is_empty()
        );
    }
}
