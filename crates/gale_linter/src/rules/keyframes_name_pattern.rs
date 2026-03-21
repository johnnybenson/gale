use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};
use regex::Regex;

use crate::rule::{Rule, RuleContext};

const DEFAULT_PATTERN: &str = "^([a-z][a-z0-9]*)(-[a-z0-9]+)*$";

/// Enforces a naming pattern for keyframes names.
///
/// Checks `@keyframes` and `@-webkit-keyframes` at-rules.
/// Accepts a regex string as the primary option.
/// Defaults to kebab-case pattern if no option is provided.
///
/// Equivalent to Stylelint's `keyframes-name-pattern` rule.
pub struct KeyframesNamePattern;

impl Rule for KeyframesNamePattern {
    fn name(&self) -> &'static str {
        "keyframes-name-pattern"
    }

    fn description(&self) -> &'static str {
        "Specify a pattern for keyframes names"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::AtRule(at) = node else {
            return vec![];
        };

        let lower_name = at.name.to_ascii_lowercase();
        if lower_name != "keyframes" && lower_name != "-webkit-keyframes" {
            return vec![];
        }

        let pattern_str = ctx
            .options
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_PATTERN);

        let re = match Regex::new(pattern_str) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        let name = at.params.trim();
        if name.is_empty() {
            return vec![];
        }

        // Strip optional quotes around the keyframes name
        let name = name
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .or_else(|| {
                name.strip_prefix('\'')
                    .and_then(|s| s.strip_suffix('\''))
            })
            .unwrap_or(name);

        if !re.is_match(name) {
            return vec![
                Diagnostic::new(
                    self.name(),
                    format!(
                        "Expected keyframes name \"{name}\" to match pattern \"{pattern_str}\""
                    ),
                )
                .severity(self.default_severity())
                .span(Span::new(at.span.offset, at.span.length)),
            ];
        }

        vec![]
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

    fn ctx_with_options(opts: &serde_json::Value) -> RuleContext<'_> {
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: Some(opts),
        }
    }

    fn keyframes(name: &str, params: &str) -> CssNode {
        CssNode::AtRule(AtRule {
            name: name.to_string(),
            params: params.to_string(),
            span: ParserSpan::new(0, 0),
            children: vec![],
        })
    }

    #[test]
    fn reports_camel_case_keyframes() {
        let d = KeyframesNamePattern.check(&keyframes("keyframes", "fadeIn"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("fadeIn"));
    }

    #[test]
    fn allows_kebab_case_keyframes() {
        let d = KeyframesNamePattern.check(&keyframes("keyframes", "fade-in"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn checks_webkit_keyframes() {
        let d =
            KeyframesNamePattern.check(&keyframes("-webkit-keyframes", "slideIn"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("slideIn"));
    }

    #[test]
    fn ignores_non_keyframes_at_rule() {
        let d = KeyframesNamePattern.check(&keyframes("media", "(min-width: 768px)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn custom_pattern() {
        let opts = serde_json::json!("^[a-z][a-zA-Z0-9]+$");
        let c = ctx_with_options(&opts);
        // camelCase should pass
        assert!(
            KeyframesNamePattern
                .check(&keyframes("keyframes", "fadeIn"), &c)
                .is_empty()
        );
        // kebab-case should fail
        let d = KeyframesNamePattern.check(&keyframes("keyframes", "fade-in"), &c);
        assert_eq!(d.len(), 1);
    }
}
