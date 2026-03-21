use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Edit, Fix, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports vendor-prefixed properties (e.g. `-webkit-transform`).
///
/// Equivalent to Stylelint's `property-no-vendor-prefix` rule.
pub struct PropertyNoVendorPrefix;

impl Rule for PropertyNoVendorPrefix {
    fn name(&self) -> &'static str {
        "property-no-vendor-prefix"
    }

    fn description(&self) -> &'static str {
        "Disallow vendor prefixes for properties"
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
            if is_vendor_prefixed(&decl.property) {
                let unprefixed = strip_vendor_prefix(&decl.property);

                // Try to find the property in the source to build a fix
                let decl_start = decl.span.offset;
                let decl_end = decl_start + decl.span.length;
                let fix =
                    if decl_end <= ctx.source.len() && decl_start < decl_end {
                        let search_area = &ctx.source[decl_start..decl_end];
                        let lower_search = search_area.to_ascii_lowercase();
                        let lower_prop = decl.property.to_ascii_lowercase();
                        lower_search.find(&lower_prop).map(|rel_offset| {
                            let abs_offset = decl_start + rel_offset;
                            Fix::new(
                                format!("Remove vendor prefix from \"{}\"", decl.property),
                                vec![Edit::new(
                                    Span::new(abs_offset, decl.property.len()),
                                    &unprefixed,
                                )],
                            )
                        })
                    } else {
                        None
                    };

                let mut diag = Diagnostic::new(
                    self.name(),
                    format!(
                        "Unexpected vendor-prefixed property \"{}\"",
                        decl.property
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

fn strip_vendor_prefix(property: &str) -> String {
    let p = property.to_ascii_lowercase();
    for prefix in &["-webkit-", "-moz-", "-ms-", "-o-"] {
        if p.starts_with(prefix) {
            return property[prefix.len()..].to_string();
        }
    }
    property.to_string()
}

fn is_vendor_prefixed(property: &str) -> bool {
    let p = property.to_ascii_lowercase();
    // Custom properties (--) are not vendor prefixes
    if p.starts_with("--") {
        return false;
    }
    p.starts_with("-webkit-")
        || p.starts_with("-moz-")
        || p.starts_with("-ms-")
        || p.starts_with("-o-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css }
    }

    fn style_decl(prop: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: prop.to_string(),
                value: "none".to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_webkit_prefix() {
        let d = PropertyNoVendorPrefix.check(&style_decl("-webkit-transform"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("-webkit-transform"));
    }

    #[test]
    fn allows_standard_property() {
        assert!(PropertyNoVendorPrefix.check(&style_decl("transform"), &ctx()).is_empty());
    }

    #[test]
    fn allows_custom_property() {
        assert!(PropertyNoVendorPrefix.check(&style_decl("--my-var"), &ctx()).is_empty());
    }

    #[test]
    fn emits_fix_for_vendor_prefixed_property() {
        let source = "a { -webkit-transform: none; }";
        let ctx = RuleContext { file_path: "t.css", source, syntax: Syntax::Css };
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "-webkit-transform".to_string(),
                value: "none".to_string(),
                span: ParserSpan::new(4, 24),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, source.len()),
        });
        let d = PropertyNoVendorPrefix.check(&node, &ctx);
        assert_eq!(d.len(), 1);
        assert!(d[0].fix.is_some());
        let fix = d[0].fix.as_ref().unwrap();
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].new_text, "transform");
    }
}
