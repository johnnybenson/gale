use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Limit the number of type selectors in a selector.
///
/// Equivalent to Stylelint's `selector-max-type` rule.
/// Default maximum: 3. Counts bare element names (type selectors). Detection-only.
pub struct SelectorMaxType;

const MAX_TYPE: usize = 3;

impl Rule for SelectorMaxType {
    fn name(&self) -> &'static str {
        "selector-max-type"
    }

    fn description(&self) -> &'static str {
        "Limit the number of type selectors in a selector"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        // Read configured max from options (primary option is a number).
        let max = ctx
            .options
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(MAX_TYPE);

        let selector = if matches!(ctx.syntax, gale_css_parser::Syntax::Scss | gale_css_parser::Syntax::Sass | gale_css_parser::Syntax::Less) {
            strip_scss_constructs(&rule.selector)
        } else {
            rule.selector.clone()
        };

        let mut diags = Vec::new();
        // Check each comma-separated selector individually
        for sel in selector.split(',') {
            let sel = sel.trim();
            if sel.is_empty() {
                continue;
            }
            let count = count_type_selectors(sel);
            if count > max {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected selector \"{sel}\" to have no more than {max} type selector(s), found {count}",
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

/// Count type selectors (bare element names like `div`, `span`, `a`).
///
/// A type selector is an identifier that is not preceded by `.`, `#`, `:`, `@`, `[`, or `*`.
/// We split by combinators and whitespace and examine each simple selector segment.
fn count_type_selectors(selector: &str) -> usize {
    let mut count = 0;
    let chars: Vec<char> = selector.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Skip whitespace and combinators.
        while i < len
            && (chars[i].is_ascii_whitespace()
                || chars[i] == '>'
                || chars[i] == '+'
                || chars[i] == '~'
                || chars[i] == ',')
        {
            i += 1;
        }
        if i >= len {
            break;
        }

        // Determine what kind of simple selector starts here.
        match chars[i] {
            '.' | '#' | '*' => {
                // Class, ID, or universal selector - skip the ident.
                i += 1;
                while i < len && is_ident_char(chars[i]) {
                    i += 1;
                }
            }
            ':' => {
                // Pseudo-class/element - skip through.
                i += 1;
                if i < len && chars[i] == ':' {
                    i += 1;
                }
                // Skip ident.
                while i < len && is_ident_char(chars[i]) {
                    i += 1;
                }
                // Skip parenthesized args if present.
                if i < len && chars[i] == '(' {
                    let mut depth = 1;
                    i += 1;
                    while i < len && depth > 0 {
                        if chars[i] == '(' {
                            depth += 1;
                        } else if chars[i] == ')' {
                            depth -= 1;
                        }
                        i += 1;
                    }
                }
            }
            '[' => {
                // Attribute selector - skip until ].
                while i < len && chars[i] != ']' {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
            }
            _ if chars[i].is_ascii_alphabetic() || !chars[i].is_ascii() => {
                // This looks like a type selector.
                count += 1;
                while i < len && is_ident_char(chars[i]) {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    count
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || !c.is_ascii()
}

/// Strip SCSS-specific constructs from a selector string:
/// - `//` line comments
/// - `#{...}` interpolation (replaced with empty string)
fn strip_scss_constructs(selector: &str) -> String {
    // First strip line comments
    let no_comments: String = selector
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    // Then strip #{...} interpolation
    let mut result = String::with_capacity(no_comments.len());
    let chars: Vec<char> = no_comments.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if i + 1 < len && chars[i] == '#' && chars[i + 1] == '{' {
            // Skip #{...}
            let mut depth = 1;
            i += 2;
            while i < len && depth > 0 {
                if chars[i] == '{' {
                    depth += 1;
                } else if chars[i] == '}' {
                    depth -= 1;
                }
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css, options: None }
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
    fn reports_too_many_type_selectors() {
        let d = SelectorMaxType.check(&style_with_selector("div span a ul"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("found 4"));
    }

    #[test]
    fn allows_within_limit() {
        let d = SelectorMaxType.check(&style_with_selector("div span a"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn does_not_count_class_selectors() {
        let d = SelectorMaxType.check(&style_with_selector(".a .b .c .d"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn does_not_count_id_selectors() {
        let d = SelectorMaxType.check(&style_with_selector("#a #b #c #d"), &ctx());
        assert!(d.is_empty());
    }
}
