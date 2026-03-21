use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Prefer range notation for media features (e.g. `width >= 768px` instead of `min-width: 768px`).
///
/// Equivalent to Stylelint's `media-feature-range-notation` rule with "context" option.
/// Detection-only.
pub struct MediaFeatureRangeNotation;

/// Prefixes that indicate the old min/max notation.
const RANGE_PREFIXES: &[&str] = &["min-", "max-"];

/// Media features that support range notation.
const RANGE_FEATURES: &[&str] = &[
    "width",
    "height",
    "device-width",
    "device-height",
    "aspect-ratio",
    "device-aspect-ratio",
    "color",
    "color-index",
    "monochrome",
    "resolution",
];

impl Rule for MediaFeatureRangeNotation {
    fn name(&self) -> &'static str {
        "media-feature-range-notation"
    }

    fn description(&self) -> &'static str {
        "Specify context or prefix notation for media feature ranges"
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

        let params_lower = at.params.to_ascii_lowercase();
        let mut diags = Vec::new();

        for &prefix in RANGE_PREFIXES {
            for &feature in RANGE_FEATURES {
                let prefixed = format!("{prefix}{feature}");
                if params_lower.contains(&prefixed) {
                    diags.push(
                        Diagnostic::new(
                            self.name(),
                            format!(
                                "Expected range notation instead of \"{prefixed}\" prefix notation"
                            ),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(at.span.offset, at.span.length)),
                    );
                }
            }
        }
        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{AtRule, Span as ParserSpan, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        }
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
    fn reports_min_width_prefix() {
        let d = MediaFeatureRangeNotation.check(&media("(min-width: 768px)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("min-width"));
    }

    #[test]
    fn reports_max_height_prefix() {
        let d = MediaFeatureRangeNotation.check(&media("(max-height: 600px)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("max-height"));
    }

    #[test]
    fn allows_range_notation() {
        let d = MediaFeatureRangeNotation.check(&media("(width >= 768px)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_non_range_features() {
        let d = MediaFeatureRangeNotation.check(&media("(hover: hover)"), &ctx());
        assert!(d.is_empty());
    }
}
