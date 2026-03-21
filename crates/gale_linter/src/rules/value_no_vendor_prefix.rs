use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports vendor-prefixed values (e.g. `-webkit-flex`).
///
/// Equivalent to Stylelint's `value-no-vendor-prefix` rule.
pub struct ValueNoVendorPrefix;

const VENDOR_PREFIXES: &[&str] = &["-webkit-", "-moz-", "-ms-", "-o-"];

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

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let mut diags = Vec::new();
        for decl in &rule.declarations {
            let lower = decl.value.to_ascii_lowercase();
            for prefix in VENDOR_PREFIXES {
                if lower.contains(prefix) {
                    diags.push(
                        Diagnostic::new(
                            self.name(),
                            format!(
                                "Unexpected vendor-prefixed value \"{}\"",
                                decl.value
                            ),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(decl.span.offset, decl.span.length)),
                    );
                    break;
                }
            }
        }
        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css }
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
        assert!(ValueNoVendorPrefix.check(&style_decl("flex"), &ctx()).is_empty());
    }
}
