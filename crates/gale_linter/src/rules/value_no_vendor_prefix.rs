use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Edit, Fix, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports vendor-prefixed values (e.g. `-webkit-flex`).
///
/// Equivalent to Stylelint's `value-no-vendor-prefix` rule.
pub struct ValueNoVendorPrefix;

const VENDOR_PREFIXES: &[&str] = &["-webkit-", "-moz-", "-ms-", "-o-"];

/// Known CSS values that have standard unprefixed equivalents (from Autoprefixer data).
/// Only vendor-prefixed versions of these values should be flagged.
const KNOWN_PREFIXABLE_VALUES: &[&str] = &[
    "box",            // display: -webkit-box
    "calc",           // -webkit-calc()
    "cross-fade",     // -webkit-cross-fade()
    "element",        // -moz-element()
    "fill-available", // -webkit-fill-available
    "fit-content",    // -webkit-fit-content
    "flex",           // display: -webkit-flex
    "flexbox",        // display: -ms-flexbox (old IE syntax)
    "grab",           // cursor: -webkit-grab
    "grabbing",       // cursor: -webkit-grabbing
    "image-set",      // -webkit-image-set()
    "inline-box",     // display: -webkit-inline-box
    "inline-flex",    // display: -webkit-inline-flex
    "isolate",        // unicode-bidi: -moz-isolate
    "linear-gradient",
    "max-content", // width: -webkit-max-content
    "min-content", // width: -webkit-min-content
    "plaintext",   // unicode-bidi: -moz-plaintext
    "radial-gradient",
    "repeating-linear-gradient",
    "repeating-radial-gradient",
    "sticky",   // position: -webkit-sticky
    "zoom-in",  // cursor: -webkit-zoom-in
    "zoom-out", // cursor: -webkit-zoom-out
];

impl Rule for ValueNoVendorPrefix {
    fn name(&self) -> &'static str {
        "value-no-vendor-prefix"
    }

    fn description(&self) -> &'static str {
        "Disallow vendor prefixes for values"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        // Read ignoreValues from options (secondary option object).
        let ignore_values: Vec<String> = ctx
            .options
            .and_then(|v| v.get("ignoreValues"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_str().map(|s| s.to_ascii_lowercase()))
                    .collect()
            })
            .unwrap_or_default();

        let mut diags = Vec::new();
        for decl in &rule.declarations {
            let lower = decl.value.to_ascii_lowercase();
            for prefix in VENDOR_PREFIXES {
                if lower.contains(prefix) {
                    // Extract the unprefixed value part
                    let unprefixed = extract_unprefixed_value(&lower, prefix);

                    // Only flag known autoprefixable values
                    if !KNOWN_PREFIXABLE_VALUES.iter().any(|v| *v == unprefixed) {
                        continue;
                    }

                    // Check if the unprefixed value is in the ignore list
                    if ignore_values.iter().any(|v| v == &unprefixed) {
                        continue;
                    }
                    // Try to build a fix by finding the prefixed value in source
                    let decl_start = decl.span.offset;
                    let decl_end = decl_start + decl.span.length;
                    let fix = if decl_end <= ctx.source.len() && decl_start < decl_end {
                        let search_area = &ctx.source[decl_start..decl_end];
                        let lower_search = search_area.to_ascii_lowercase();
                        // Find the vendor prefix in the source and remove it
                        lower_search.find(prefix).map(|rel_offset| {
                            let abs_offset = decl_start + rel_offset;
                            Fix::new(
                                format!(
                                    "Remove vendor prefix \"{}\"",
                                    prefix.trim_end_matches('-')
                                ),
                                vec![Edit::new(Span::new(abs_offset, prefix.len()), "")],
                            )
                        })
                    } else {
                        None
                    };

                    let mut diag = Diagnostic::new(
                        self.name(),
                        format!("Unexpected vendor-prefixed value \"{}\"", decl.value),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(decl.span.offset, decl.span.length));

                    if let Some(f) = fix {
                        diag = diag.fix(f);
                    }

                    diags.push(diag);
                    break;
                }
            }
        }
        diags
    }
}

/// Extract the unprefixed value identifier from a CSS value containing a vendor prefix.
/// E.g. "-webkit-flex" -> "flex", "-webkit-match-parent" -> "match-parent"
fn extract_unprefixed_value(lower_value: &str, prefix: &str) -> String {
    // Find the prefix in the value and return what follows it
    if let Some(pos) = lower_value.find(prefix) {
        let after = &lower_value[pos + prefix.len()..];
        // Take until whitespace, comma, or paren
        let end = after
            .find(|c: char| c.is_ascii_whitespace() || c == ',' || c == ')' || c == '(')
            .unwrap_or(after.len());
        after[..end].to_string()
    } else {
        lower_value.to_string()
    }
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

    fn style_decl(val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "display".to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_webkit_prefix_value() {
        let d = ValueNoVendorPrefix.check(&style_decl("-webkit-flex"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("-webkit-flex"));
    }

    #[test]
    fn reports_ms_prefix_value() {
        let d = ValueNoVendorPrefix.check(&style_decl("-ms-flexbox"), &ctx());
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn allows_standard_value() {
        assert!(
            ValueNoVendorPrefix
                .check(&style_decl("flex"), &ctx())
                .is_empty()
        );
    }

    #[test]
    fn emits_fix_for_vendor_prefixed_value() {
        let source = "a { display: -webkit-flex; }";
        let ctx = RuleContext {
            file_path: "t.css",
            source,
            syntax: Syntax::Css,
            options: None,
        };
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "display".to_string(),
                value: "-webkit-flex".to_string(),
                span: ParserSpan::new(4, 22),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, source.len()),
        });
        let d = ValueNoVendorPrefix.check(&node, &ctx);
        assert_eq!(d.len(), 1);
        assert!(d[0].fix.is_some());
        let fix = d[0].fix.as_ref().unwrap();
        assert_eq!(fix.edits.len(), 1);
        // The fix removes the "-webkit-" prefix, leaving "flex"
        assert_eq!(fix.edits[0].new_text, "");
        assert_eq!(fix.edits[0].span.length, "-webkit-".len());
    }
}
