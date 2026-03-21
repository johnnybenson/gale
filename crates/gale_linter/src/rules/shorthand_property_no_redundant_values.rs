use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Edit, Fix, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports shorthand property values that contain redundant parts
/// (e.g. `margin: 1px 1px 1px 1px` → `margin: 1px`).
///
/// Equivalent to Stylelint's `shorthand-property-no-redundant-values` rule.
pub struct ShorthandPropertyNoRedundantValues;

const SHORTHAND_PROPERTIES: &[&str] = &[
    "margin",
    "padding",
    "border-color",
    "border-style",
    "border-width",
    "border-radius",
    "gap",
    "grid-gap",
    "overflow",
    "inset",
];

impl Rule for ShorthandPropertyNoRedundantValues {
    fn name(&self) -> &'static str {
        "shorthand-property-no-redundant-values"
    }

    fn description(&self) -> &'static str {
        "Disallow redundant values in shorthand properties"
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
            let prop = decl.property.to_ascii_lowercase();
            if !SHORTHAND_PROPERTIES.contains(&prop.as_str()) {
                continue;
            }
            let parts: Vec<&str> = decl.value.split_whitespace().collect();
            if let Some(shortened) = shorten(&parts) {
                // Build a fix: find the value in the source and replace it.
                let fix = find_value_and_build_fix(ctx.source, decl, &shortened);

                let mut diag = Diagnostic::new(
                    self.name(),
                    format!(
                        "Expected \"{}\" instead of \"{}\"",
                        shortened, decl.value
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
        diags
    }
}

/// Find the value portion of a declaration in source and build a Fix.
fn find_value_and_build_fix(
    source: &str,
    decl: &gale_css_parser::Declaration,
    shortened: &str,
) -> Option<Fix> {
    // The declaration span covers `property: value`. Find the value part
    // by searching for the original value string within the span.
    let decl_start = decl.span.offset;
    let decl_end = decl_start + decl.span.length;
    if decl_end > source.len() {
        return None;
    }
    let decl_text = &source[decl_start..decl_end];

    // Find the colon, then the value starts after it (skipping whitespace).
    let colon_pos = decl_text.find(':')?;
    let after_colon = &decl_text[colon_pos + 1..];
    let leading_ws = after_colon.len() - after_colon.trim_start().len();
    let value_start_in_source = decl_start + colon_pos + 1 + leading_ws;

    // The value ends before any trailing semicolon/whitespace/!important.
    let trimmed_val = after_colon.trim();
    // Strip trailing semicolon if present.
    let val_text = trimmed_val.trim_end_matches(';').trim_end_matches("!important").trim();
    let value_end_in_source = value_start_in_source + val_text.len();

    Some(Fix::new(
        format!("Shorten to \"{shortened}\""),
        vec![Edit::new(
            Span::from_range(value_start_in_source, value_end_in_source),
            shortened,
        )],
    ))
}

/// Try to shorten redundant values. Returns Some(shortened) if redundant.
fn shorten(parts: &[&str]) -> Option<String> {
    match parts.len() {
        4 => {
            let (top, right, bottom, left) = (parts[0], parts[1], parts[2], parts[3]);
            if top == right && right == bottom && bottom == left {
                // 1px 1px 1px 1px → 1px
                Some(top.to_string())
            } else if top == bottom && right == left {
                // 1px 2px 1px 2px → 1px 2px
                Some(format!("{top} {right}"))
            } else if right == left {
                // 1px 2px 3px 2px → 1px 2px 3px
                Some(format!("{top} {right} {bottom}"))
            } else {
                None
            }
        }
        3 => {
            let (top, right, bottom) = (parts[0], parts[1], parts[2]);
            if top == right && right == bottom {
                Some(top.to_string())
            } else if top == bottom {
                Some(format!("{top} {right}"))
            } else {
                None
            }
        }
        2 => {
            if parts[0] == parts[1] {
                Some(parts[0].to_string())
            } else {
                None
            }
        }
        _ => None,
    }
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
    fn reports_four_identical_values() {
        let d = ShorthandPropertyNoRedundantValues.check(
            &style_decl("margin", "1px 1px 1px 1px"),
            &ctx(),
        );
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("1px"));
    }

    #[test]
    fn reports_two_identical_values() {
        let d = ShorthandPropertyNoRedundantValues.check(
            &style_decl("padding", "10px 10px"),
            &ctx(),
        );
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn allows_non_redundant_values() {
        assert!(
            ShorthandPropertyNoRedundantValues
                .check(&style_decl("margin", "1px 2px 3px 4px"), &ctx())
                .is_empty()
        );
        assert!(
            ShorthandPropertyNoRedundantValues
                .check(&style_decl("margin", "1px"), &ctx())
                .is_empty()
        );
    }

    #[test]
    fn fix_shortens_four_identical() {
        let source = "a { margin: 1px 1px 1px 1px; }";
        let rule = ShorthandPropertyNoRedundantValues;
        // Build node with correct span covering the declaration.
        let decl_start = 4; // "margin: 1px 1px 1px 1px;"
        let decl_text = "margin: 1px 1px 1px 1px;";
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "margin".to_string(),
                value: "1px 1px 1px 1px".to_string(),
                span: ParserSpan::new(decl_start, decl_text.len()),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, source.len()),
        });
        let context = RuleContext {
            file_path: "t.css",
            source,
            syntax: Syntax::Css,
        };
        let diags = rule.check(&node, &context);
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("should have a fix");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].new_text, "1px");
        // Apply fix and verify
        let (fixed, count) = gale_diagnostics::apply_fixes(source, &diags);
        assert_eq!(count, 1);
        assert_eq!(fixed, "a { margin: 1px; }");
    }

    #[test]
    fn fix_shortens_two_identical() {
        let source = "a { padding: 10px 10px; }";
        let decl_start = 4;
        let decl_text = "padding: 10px 10px;";
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "padding".to_string(),
                value: "10px 10px".to_string(),
                span: ParserSpan::new(decl_start, decl_text.len()),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, source.len()),
        });
        let context = RuleContext {
            file_path: "t.css",
            source,
            syntax: Syntax::Css,
        };
        let diags = ShorthandPropertyNoRedundantValues.check(&node, &context);
        assert_eq!(diags.len(), 1);
        let (fixed, count) = gale_diagnostics::apply_fixes(source, &diags);
        assert_eq!(count, 1);
        assert_eq!(fixed, "a { padding: 10px; }");
    }
}
