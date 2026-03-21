use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Edit, Fix, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Enforce lowercase for CSS keyword values (e.g. `Inherit` → `inherit`).
///
/// Equivalent to Stylelint's `value-keyword-case` rule with "lower" option.
pub struct ValueKeywordCase;

/// Known CSS keyword values that should be lowercase.
const KEYWORDS: &[&str] = &[
    "inherit",
    "initial",
    "unset",
    "revert",
    "revert-layer",
    "none",
    "auto",
    "block",
    "inline",
    "inline-block",
    "flex",
    "inline-flex",
    "grid",
    "inline-grid",
    "table",
    "hidden",
    "visible",
    "absolute",
    "relative",
    "fixed",
    "sticky",
    "static",
    "bold",
    "bolder",
    "lighter",
    "normal",
    "italic",
    "oblique",
    "uppercase",
    "lowercase",
    "capitalize",
    "underline",
    "overline",
    "line-through",
    "nowrap",
    "wrap",
    "collapse",
    "separate",
    "transparent",
    "currentcolor",
    "pointer",
    "default",
    "solid",
    "dashed",
    "dotted",
    "double",
    "center",
    "left",
    "right",
    "top",
    "bottom",
    "baseline",
    "stretch",
    "cover",
    "contain",
    "scroll",
    "smooth",
];

impl Rule for ValueKeywordCase {
    fn name(&self) -> &'static str {
        "value-keyword-case"
    }

    fn description(&self) -> &'static str {
        "Enforce lowercase for keyword values"
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
            let search_area = if decl_end <= ctx.source.len() && decl_start < decl_end {
                &ctx.source[decl_start..decl_end]
            } else {
                &decl.value
            };

            for kw in KEYWORDS {
                for m in find_keyword_wrong_case(search_area, kw) {
                    let abs_offset = if decl_end <= ctx.source.len() && decl_start < decl_end {
                        decl_start + m.0
                    } else {
                        decl_start
                    };
                    diags.push(
                        Diagnostic::new(
                            self.name(),
                            format!("Expected \"{}\" to be \"{}\"", m.1, kw),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(abs_offset, m.1.len()))
                        .fix(Fix::new(
                            format!("Convert to lowercase \"{kw}\""),
                            vec![Edit::new(Span::new(abs_offset, m.1.len()), *kw)],
                        )),
                    );
                }
            }
        }
        diags
    }
}

/// Find occurrences of a keyword with wrong case (case-insensitive match but not all lowercase).
/// Returns (relative_offset, matched_text).
fn find_keyword_wrong_case<'a>(haystack: &'a str, keyword: &str) -> Vec<(usize, &'a str)> {
    let mut results = Vec::new();
    let lower_haystack = haystack.to_ascii_lowercase();
    let mut start = 0;
    while let Some(pos) = lower_haystack[start..].find(keyword) {
        let abs_pos = start + pos;
        let end = abs_pos + keyword.len();
        // Check word boundaries: the match must not be part of a longer identifier.
        let before_ok = abs_pos == 0
            || !haystack.as_bytes()[abs_pos - 1].is_ascii_alphanumeric()
                && haystack.as_bytes()[abs_pos - 1] != b'-'
                && haystack.as_bytes()[abs_pos - 1] != b'_';
        let after_ok = end >= haystack.len()
            || !haystack.as_bytes()[end].is_ascii_alphanumeric()
                && haystack.as_bytes()[end] != b'-'
                && haystack.as_bytes()[end] != b'_';
        if before_ok && after_ok {
            let matched = &haystack[abs_pos..end];
            // Only report if the matched text differs from the expected lowercase form.
            if matched != keyword {
                results.push((abs_pos, matched));
            }
        }
        start = abs_pos + 1;
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
            syntax: Syntax::Css, options: None }
    }

    fn style_with_decl(prop: &str, val: &str) -> CssNode {
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
    fn reports_uppercase_keyword() {
        let d = ValueKeywordCase.check(&style_with_decl("display", "BLOCK"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("\"BLOCK\""));
        assert!(d[0].message.contains("\"block\""));
        assert!(d[0].fix.is_some());
    }

    #[test]
    fn allows_lowercase_keyword() {
        let d = ValueKeywordCase.check(&style_with_decl("display", "block"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn reports_mixed_case() {
        let d = ValueKeywordCase.check(&style_with_decl("color", "Inherit"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("\"Inherit\""));
    }
}
