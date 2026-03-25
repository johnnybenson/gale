use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Specify context or prefix notation for media feature ranges.
///
/// Equivalent to Stylelint's `media-feature-range-notation` rule.
///
/// Primary option:
///   - `"context"` (default): Expect range notation (e.g. `width >= 768px`)
///   - `"prefix"`: Expect prefix notation (e.g. `min-width: 768px`)
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

/// Range comparison operators used in context notation.
const RANGE_OPS: &[&str] = &[">=", "<=", ">", "<"];

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

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::AtRule(at) = node else {
            return vec![];
        };
        if at.name != "media" {
            return vec![];
        }

        // Skip media queries containing SCSS/Less variables, interpolation,
        // or SCSS math expressions — the actual values are unknown until
        // compilation, so range notation cannot be determined.
        if at.params.contains('$') || at.params.contains("#{") || at.params.contains("@{") {
            return vec![];
        }
        // In SCSS/Less, skip if params contain SCSS math operators (like `(124px + 300px)`)
        if ctx.syntax != gale_css_parser::Syntax::Css
            && (at.params.contains(" + ") || at.params.contains(" - ") || at.params.contains(" * "))
        {
            return vec![];
        }

        let notation = parse_notation(ctx.options);

        // If "prefix" notation, skip entirely — Gale does not currently detect
        // range→prefix violations (requires parsing range syntax which is rare).
        if notation == Notation::Prefix {
            return vec![];
        }

        // Determine the notation option string for the message
        let notation_str = ctx
            .options
            .and_then(|v| v.as_str())
            .or_else(|| {
                ctx.options
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("context");

        let params_lower = at.params.to_ascii_lowercase();
        let mut diags = Vec::new();

        let at_src_end = (at.span.offset + at.span.length).min(ctx.source.len());
        let at_src = &ctx.source[at.span.offset..at_src_end];
        let at_src_lower = at_src.to_ascii_lowercase();
        for &prefix in RANGE_PREFIXES {
            for &feature in RANGE_FEATURES {
                let prefixed = format!("{prefix}{feature}");
                if params_lower.contains(&prefixed) {
                    // Stylelint points to the `(` before the min-/max- feature name.
                    let paren_search = format!("({prefixed}");
                    let paren_off = at_src_lower
                        .find(&paren_search)
                        .or_else(|| {
                            // fallback: find `(` before the feature in source
                            at_src_lower.find(&prefixed).and_then(|p| {
                                at_src_lower[..p].rfind('(')
                            })
                        })
                        .unwrap_or(0);
                    diags.push(
                        Diagnostic::new(
                            self.name(),
                            format!("Expected \"{notation_str}\" media feature range notation"),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(at.span.offset + paren_off, 1)),
                    );
                }
            }
        }
        diags
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Notation {
    Context,
    Prefix,
}

fn parse_notation(options: Option<&serde_json::Value>) -> Notation {
    let Some(value) = options else {
        return Notation::Context;
    };
    match value {
        serde_json::Value::String(s) => {
            if s == "prefix" {
                Notation::Prefix
            } else {
                Notation::Context
            }
        }
        serde_json::Value::Array(arr) => {
            if let Some(s) = arr.first().and_then(|v| v.as_str())
                && s == "prefix"
            {
                return Notation::Prefix;
            }
            Notation::Context
        }
        _ => Notation::Context,
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
        assert!(d[0].message.contains("media feature range notation"));
    }

    #[test]
    fn allows_range_syntax() {
        let d = MediaFeatureRangeNotation.check(&media("(width >= 768px)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn skips_scss_variables() {
        let d = MediaFeatureRangeNotation.check(&media("(min-width: $breakpoint)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn prefix_notation_allows_prefix() {
        let opts = serde_json::json!("prefix");
        let ctx = RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: Some(&opts),
        };
        let d = MediaFeatureRangeNotation.check(&media("(min-width: 768px)"), &ctx);
        assert!(d.is_empty());
    }
}
