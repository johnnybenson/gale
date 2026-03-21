use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow vendor prefixes in media feature names.
///
/// Equivalent to Stylelint's `media-feature-name-no-vendor-prefix` rule.
/// E.g., `@media (-webkit-min-device-pixel-ratio: 2)` should be flagged.
pub struct MediaFeatureNameNoVendorPrefix;

const VENDOR_PREFIXES: &[&str] = &["-webkit-", "-moz-", "-ms-", "-o-"];

impl Rule for MediaFeatureNameNoVendorPrefix {
    fn name(&self) -> &'static str {
        "media-feature-name-no-vendor-prefix"
    }

    fn description(&self) -> &'static str {
        "Disallow vendor prefixes for media feature names"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::AtRule(rule) = node else {
            return vec![];
        };

        // Only check @media rules
        if !rule.name.eq_ignore_ascii_case("media") {
            return vec![];
        }

        let params_lower = rule.params.to_ascii_lowercase();
        let mut diags = Vec::new();

        for prefix in VENDOR_PREFIXES {
            if params_lower.contains(prefix) {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Unexpected vendor-prefixed media feature name in \"@media {}\"",
                            rule.params.trim()
                        ),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(rule.span.offset, rule.span.length)),
                );
                break; // one diagnostic per at-rule
            }
        }

        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{AtRule as CssAtRule, Span as ParserSpan, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        }
    }

    fn media_rule(params: &str) -> CssNode {
        CssNode::AtRule(CssAtRule {
            name: "media".to_string(),
            params: params.to_string(),
            span: ParserSpan::new(0, 0),
            children: vec![],
        })
    }

    #[test]
    fn reports_webkit_device_pixel_ratio() {
        let d = MediaFeatureNameNoVendorPrefix
            .check(&media_rule("(-webkit-min-device-pixel-ratio: 2)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("-webkit-"));
    }

    #[test]
    fn reports_moz_prefix() {
        let d = MediaFeatureNameNoVendorPrefix
            .check(&media_rule("(-moz-device-pixel-ratio: 2)"), &ctx());
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn allows_standard_media_features() {
        let d = MediaFeatureNameNoVendorPrefix.check(&media_rule("(min-width: 768px)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_non_media_at_rule() {
        let node = CssNode::AtRule(CssAtRule {
            name: "keyframes".to_string(),
            params: "-webkit-fade".to_string(),
            span: ParserSpan::new(0, 0),
            children: vec![],
        });
        let d = MediaFeatureNameNoVendorPrefix.check(&node, &ctx());
        assert!(d.is_empty());
    }
}
