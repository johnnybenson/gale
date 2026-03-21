use gale_css_parser::{CssNode, Syntax};
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow leading underscore in partial names within `@use`, `@forward`,
/// and `@import`.
pub struct ScssLoadNoPartialLeadingUnderscore;

impl Rule for ScssLoadNoPartialLeadingUnderscore {
    fn name(&self) -> &'static str {
        "scss/load-no-partial-leading-underscore"
    }

    fn description(&self) -> &'static str {
        "Disallow leading underscore in partial names"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        if !matches!(ctx.syntax, Syntax::Scss | Syntax::Sass) {
            return vec![];
        }

        let CssNode::AtRule(at) = node else {
            return vec![];
        };

        if !matches!(at.name.as_str(), "use" | "forward" | "import") {
            return vec![];
        }

        // Extract the path from params. Strip quotes and whitespace.
        let path = extract_path(&at.params);
        if path.is_empty() {
            return vec![];
        }

        // Get the filename portion (last segment after `/`).
        let filename = path.rsplit('/').next().unwrap_or(path);
        if filename.starts_with('_') {
            vec![Diagnostic::new(
                self.name(),
                format!(
                    "Unexpected leading underscore in partial name \"{}\"",
                    path
                ),
            )
            .severity(self.default_severity())
            .span(Span::new(at.span.offset, at.span.length))]
        } else {
            vec![]
        }
    }
}

/// Extract a path string from at-rule params, stripping quotes.
fn extract_path(params: &str) -> &str {
    let trimmed = params.trim();
    // Remove quotes if present.
    let trimmed = trimmed
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
        })
        .unwrap_or(trimmed);
    // For @use, there may be ` as <name>` after the path.
    trimmed.split_whitespace().next().unwrap_or(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{AtRule, Span as ParserSpan, Syntax};

    fn scss_ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "t.scss",
            source: "",
            syntax: Syntax::Scss,
            options: None,
        }
    }

    fn use_rule(params: &str) -> CssNode {
        CssNode::AtRule(AtRule {
            name: "use".to_string(),
            params: params.to_string(),
            span: ParserSpan::new(0, 10),
            children: vec![],
        })
    }

    fn import_rule(params: &str) -> CssNode {
        CssNode::AtRule(AtRule {
            name: "import".to_string(),
            params: params.to_string(),
            span: ParserSpan::new(0, 10),
            children: vec![],
        })
    }

    #[test]
    fn reports_leading_underscore_in_use() {
        let d = ScssLoadNoPartialLeadingUnderscore.check(&use_rule("\"_variables\""), &scss_ctx());
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn reports_leading_underscore_in_import() {
        let d =
            ScssLoadNoPartialLeadingUnderscore.check(&import_rule("\"_mixins\""), &scss_ctx());
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn reports_underscore_in_path() {
        let d = ScssLoadNoPartialLeadingUnderscore
            .check(&use_rule("\"path/to/_file\""), &scss_ctx());
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn allows_no_underscore() {
        let d =
            ScssLoadNoPartialLeadingUnderscore.check(&use_rule("\"variables\""), &scss_ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_path_without_underscore() {
        let d =
            ScssLoadNoPartialLeadingUnderscore.check(&use_rule("\"path/to/file\""), &scss_ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn skips_non_scss() {
        let ctx = RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        };
        assert!(ScssLoadNoPartialLeadingUnderscore
            .check(&use_rule("\"_variables\""), &ctx)
            .is_empty());
    }
}
