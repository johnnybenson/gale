use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};
use regex::Regex;

use crate::rule::{Rule, RuleContext};

const DEFAULT_PATTERN: &str = "^[a-z][a-z0-9]*(-[a-z0-9]+)*$";

/// Enforces a pattern for class selectors.
///
/// Accepts a regex string as the primary option (e.g. `"^[a-z][a-zA-Z0-9]+$"`).
/// Defaults to kebab-case pattern if no option is provided.
///
/// Equivalent to Stylelint's `selector-class-pattern` rule.
pub struct SelectorClassPattern;

impl Rule for SelectorClassPattern {
    fn name(&self) -> &'static str {
        "selector-class-pattern"
    }

    fn description(&self) -> &'static str {
        "Specify a pattern for class selectors"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        // Read the user-supplied regex pattern from options, or use the default kebab-case pattern.
        let pattern_str = ctx
            .options
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_PATTERN);

        let re = match Regex::new(pattern_str) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        let mut diags = Vec::new();
        for class in extract_class_names(&rule.selector) {
            // Skip class names containing SCSS interpolation #{...}
            if matches!(
                ctx.syntax,
                gale_css_parser::Syntax::Scss | gale_css_parser::Syntax::Sass
            ) && class.contains("#{")
            {
                continue;
            }
            if !re.is_match(&class) {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected class selector \".{class}\" to match pattern \"{pattern_str}\""
                        ),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(rule.span.offset, rule.span.length)),
                );
            }
        }
        diags
    }
}

fn extract_class_names(selector: &str) -> Vec<String> {
    let mut classes = Vec::new();
    let chars: Vec<char> = selector.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '.' {
            i += 1;
            let start = i;
            // CSS class: ident chars (alphanum, hyphen, underscore, non-ASCII)
            // Also consume SCSS interpolation #{...} within class names
            while i < len {
                if chars[i].is_ascii_alphanumeric()
                    || chars[i] == '-'
                    || chars[i] == '_'
                    || !chars[i].is_ascii()
                {
                    i += 1;
                } else if chars[i] == '#' && i + 1 < len && chars[i + 1] == '{' {
                    // Consume SCSS interpolation #{...}
                    i += 2; // skip #{
                    let mut depth = 1;
                    while i < len && depth > 0 {
                        if chars[i] == '{' {
                            depth += 1;
                        } else if chars[i] == '}' {
                            depth -= 1;
                        }
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            if i > start {
                classes.push(chars[start..i].iter().collect());
            }
        } else {
            i += 1;
        }
    }

    classes
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

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

    fn style_with_selector(sel: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: sel.to_string(),
            declarations: vec![Declaration {
                property: "color".to_string(),
                value: "red".to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_camel_case_class() {
        let d = SelectorClassPattern.check(&style_with_selector(".myClass"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("myClass"));
    }

    #[test]
    fn allows_kebab_case_class() {
        assert!(
            SelectorClassPattern
                .check(&style_with_selector(".my-class"), &ctx())
                .is_empty()
        );
        assert!(
            SelectorClassPattern
                .check(&style_with_selector(".foo"), &ctx())
                .is_empty()
        );
    }

    #[test]
    fn reports_underscore_class() {
        let d = SelectorClassPattern.check(&style_with_selector(".my_class"), &ctx());
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn custom_pattern_camel_case() {
        let opts = serde_json::json!("^[a-z][a-zA-Z0-9]+$");
        let c = ctx_with_options(&opts);
        // camelCase should pass
        assert!(
            SelectorClassPattern
                .check(&style_with_selector(".myClass"), &c)
                .is_empty()
        );
        // kebab-case should fail
        let d = SelectorClassPattern.check(&style_with_selector(".my-class"), &c);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn custom_pattern_in_message() {
        let opts = serde_json::json!("^[a-z][a-zA-Z0-9]+$");
        let c = ctx_with_options(&opts);
        let d = SelectorClassPattern.check(&style_with_selector(".my-class"), &c);
        assert!(d[0].message.contains("^[a-z][a-zA-Z0-9]+$"));
    }
}
