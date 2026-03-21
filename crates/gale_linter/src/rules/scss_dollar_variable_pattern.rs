use gale_css_parser::{CssNode, Syntax};
use gale_diagnostics::{Diagnostic, Severity, Span};
use regex::Regex;

use crate::rule::{Rule, RuleContext};

const DEFAULT_PATTERN: &str = "^[a-z][a-z0-9]*(-[a-z0-9]+)*$";

/// Specify a pattern for SCSS `$variable` names.
///
/// Accepts a regex string as the primary option. Defaults to kebab-case.
/// The `$` prefix is stripped before matching.
///
/// Equivalent to `scss/dollar-variable-pattern`.
pub struct ScssDollarVariablePattern;

impl Rule for ScssDollarVariablePattern {
    fn name(&self) -> &'static str {
        "scss/dollar-variable-pattern"
    }

    fn description(&self) -> &'static str {
        "Specify a pattern for SCSS dollar variable names"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        if !matches!(ctx.syntax, Syntax::Scss | Syntax::Sass) {
            return vec![];
        }

        let CssNode::Declaration(decl) = node else {
            return vec![];
        };

        if !decl.property.starts_with('$') {
            return vec![];
        }

        let pattern_str = ctx.primary_option_str().unwrap_or(DEFAULT_PATTERN);
        let re = match Regex::new(pattern_str) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        // Strip the leading `$` for matching
        let var_name = &decl.property[1..];

        if var_name.is_empty() {
            return vec![];
        }

        if !re.is_match(var_name) {
            vec![
                Diagnostic::new(
                    self.name(),
                    format!(
                        "Expected ${} to match pattern \"{}\"",
                        var_name, pattern_str
                    ),
                )
                .severity(self.default_severity())
                .span(Span::new(decl.span.offset, decl.span.length)),
            ]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, Syntax};

    fn scss_ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "t.scss",
            source: "",
            syntax: Syntax::Scss,
            options: None,
        }
    }

    fn dollar_var(name: &str) -> CssNode {
        CssNode::Declaration(Declaration {
            property: name.to_string(),
            value: "red".to_string(),
            span: ParserSpan::new(0, 10),
            important: false,
        })
    }

    #[test]
    fn allows_kebab_case() {
        assert!(ScssDollarVariablePattern.check(&dollar_var("$my-var"), &scss_ctx()).is_empty());
    }

    #[test]
    fn reports_non_matching_pattern() {
        let d = ScssDollarVariablePattern.check(&dollar_var("$myVar"), &scss_ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("myVar"));
    }

    #[test]
    fn skips_non_dollar_declarations() {
        let node = CssNode::Declaration(Declaration {
            property: "color".to_string(),
            value: "red".to_string(),
            span: ParserSpan::new(0, 10),
            important: false,
        });
        assert!(ScssDollarVariablePattern.check(&node, &scss_ctx()).is_empty());
    }

    #[test]
    fn skips_non_scss() {
        let css_ctx = RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        };
        assert!(ScssDollarVariablePattern.check(&dollar_var("$myVar"), &css_ctx).is_empty());
    }
}
