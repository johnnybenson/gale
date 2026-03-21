use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Enforces kebab-case pattern for class selectors.
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
        let mut diags = Vec::new();
        for class in extract_class_names(&rule.selector) {
            // Skip class names containing SCSS interpolation #{...}
            if matches!(ctx.syntax, gale_css_parser::Syntax::Scss | gale_css_parser::Syntax::Sass)
                && class.contains("#{")
            {
                continue;
            }
            if !is_kebab_case(&class) {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected class selector \".{class}\" to match kebab-case pattern"
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

/// Matches `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`
fn is_kebab_case(name: &str) -> bool {
    let chars: Vec<char> = name.chars().collect();
    if chars.is_empty() {
        return false;
    }
    if !chars[0].is_ascii_lowercase() {
        return false;
    }

    let mut i = 1;
    while i < chars.len() {
        if chars[i] == '-' {
            i += 1;
            // Must be followed by at least one lowercase letter or digit
            if i >= chars.len() || !(chars[i].is_ascii_lowercase() || chars[i].is_ascii_digit()) {
                return false;
            }
        } else if chars[i].is_ascii_lowercase() || chars[i].is_ascii_digit() {
            // ok
        } else {
            return false;
        }
        i += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css, options: None }
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
        assert!(SelectorClassPattern.check(&style_with_selector(".my-class"), &ctx()).is_empty());
        assert!(SelectorClassPattern.check(&style_with_selector(".foo"), &ctx()).is_empty());
    }

    #[test]
    fn reports_underscore_class() {
        let d = SelectorClassPattern.check(&style_with_selector(".my_class"), &ctx());
        assert_eq!(d.len(), 1);
    }
}
