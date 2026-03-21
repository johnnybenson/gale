use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::data::is_known_media_feature;
use crate::rule::{Rule, RuleContext};

/// Reports unknown media feature names in `@media` rules.
///
/// Equivalent to Stylelint's `media-feature-name-no-unknown` rule.
pub struct MediaFeatureNameNoUnknown;

impl Rule for MediaFeatureNameNoUnknown {
    fn name(&self) -> &'static str {
        "media-feature-name-no-unknown"
    }

    fn description(&self) -> &'static str {
        "Disallow unknown media feature names"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::AtRule(at) = node else {
            return vec![];
        };
        if at.name != "media" {
            return vec![];
        }

        let mut diags = Vec::new();
        for feature in extract_media_features(&at.params) {
            // Skip vendor-prefixed features
            if feature.starts_with('-') {
                continue;
            }
            if !is_known_media_feature(&feature) {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!("Unexpected unknown media feature name \"{feature}\""),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(at.span.offset, at.span.length)),
                );
            }
        }
        diags
    }
}

/// Extract media feature names from a @media params string.
///
/// Looks for patterns like `(feature-name:` or `(feature-name)` in the
/// media query params.
fn extract_media_features(params: &str) -> Vec<String> {
    let mut features = Vec::new();
    let chars: Vec<char> = params.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '(' {
            i += 1;
            // Skip whitespace after (
            while i < len && chars[i].is_ascii_whitespace() {
                i += 1;
            }
            let start = i;
            // Collect the feature name (alphanumeric + hyphen)
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '-') {
                i += 1;
            }
            if i > start {
                // Skip whitespace
                let mut j = i;
                while j < len && chars[j].is_ascii_whitespace() {
                    j += 1;
                }
                // Must be followed by : or ) or < > = to be a feature name
                if j < len && (chars[j] == ':' || chars[j] == ')' || chars[j] == '<' || chars[j] == '>' || chars[j] == '=') {
                    let name: String = chars[start..i].iter().collect();
                    // Skip known media types that can appear in parens
                    if !matches!(name.as_str(), "not" | "and" | "or" | "only") {
                        features.push(name);
                    }
                }
            }
        } else {
            i += 1;
        }
    }

    features
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{AtRule, Span as ParserSpan, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css, options: None }
    }

    fn media(params: &str) -> CssNode {
        CssNode::AtRule(AtRule {
            name: "media".to_string(),
            params: params.to_string(),
            span: ParserSpan::new(0, 0),
            children: vec![],
        })
    }

    #[test]
    fn reports_unknown_media_feature() {
        let d = MediaFeatureNameNoUnknown.check(&media("(min-wdith: 768px)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("min-wdith"));
    }

    #[test]
    fn allows_known_media_features() {
        assert!(MediaFeatureNameNoUnknown.check(&media("(min-width: 768px)"), &ctx()).is_empty());
        assert!(MediaFeatureNameNoUnknown.check(&media("(hover: hover)"), &ctx()).is_empty());
        assert!(MediaFeatureNameNoUnknown.check(&media("(prefers-color-scheme: dark)"), &ctx()).is_empty());
    }

    #[test]
    fn allows_vendor_prefixed() {
        assert!(MediaFeatureNameNoUnknown.check(&media("(-webkit-min-device-pixel-ratio: 2)"), &ctx()).is_empty());
    }
}
