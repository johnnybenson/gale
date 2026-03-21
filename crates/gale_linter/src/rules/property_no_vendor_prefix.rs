use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

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

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let mut diags = Vec::new();
        for decl in &rule.declarations {
            if is_vendor_prefixed(&decl.property) {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Unexpected vendor-prefixed property \"{}\"",
                            decl.property
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
}
