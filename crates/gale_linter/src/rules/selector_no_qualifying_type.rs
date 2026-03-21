use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow qualifying a selector by type (e.g. `a.foo`, `div#bar`).
///
/// Equivalent to Stylelint's `selector-no-qualifying-type` rule.
pub struct SelectorNoQualifyingType;

impl Rule for SelectorNoQualifyingType {
    fn name(&self) -> &'static str {
        "selector-no-qualifying-type"
    }

    fn description(&self) -> &'static str {
        "Disallow qualifying a selector by type"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let mut diags = Vec::new();
        // Check each comma-separated selector individually
        for sel in rule.selector.split(',') {
            let sel = sel.trim();
            // Skip selectors containing SCSS interpolation — the actual selector
            // is dynamic and cannot be validated at lint time.
            if matches!(ctx.syntax, gale_css_parser::Syntax::Scss | gale_css_parser::Syntax::Sass)
                && sel.contains("#{")
            {
                continue;
            }
            if has_qualifying_type(sel) {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Unexpected qualifying type selector in \"{sel}\""
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

/// Check if a simple selector has a type selector immediately followed by a class (`.`) or ID (`#`) selector.
/// For example: `a.foo`, `div#bar`, `input.form-control`.
fn has_qualifying_type(selector: &str) -> bool {
    let chars: Vec<char> = selector.chars().collect();
    let len = chars.len();
    let mut i = 0;

    // We need to walk through the selector and find patterns like:
    // <type-selector><.class> or <type-selector><#id>
    while i < len {
        // Skip whitespace (combinators)
        if chars[i].is_ascii_whitespace() || chars[i] == '>' || chars[i] == '+' || chars[i] == '~' {
            i += 1;
            continue;
        }

        // Check if we're at the start of a type selector (letter or non-ASCII, not preceded by . # : [ *)
        if is_type_selector_start(&chars, i) {
            // Consume the type selector name
            let start = i;
            while i < len && is_ident_char(chars[i]) {
                i += 1;
            }
            // If immediately followed by . or #, it's a qualifying type
            if i > start && i < len && (chars[i] == '.' || chars[i] == '#') {
                return true;
            }
            continue;
        }

        // Skip class selectors (.foo)
        if chars[i] == '.' {
            i += 1;
            while i < len && is_ident_char(chars[i]) {
                i += 1;
            }
            continue;
        }

        // Skip ID selectors (#foo)
        if chars[i] == '#' {
            i += 1;
            while i < len && is_ident_char(chars[i]) {
                i += 1;
            }
            continue;
        }

        // Skip pseudo-classes/pseudo-elements (:hover, ::before)
        if chars[i] == ':' {
            i += 1;
            if i < len && chars[i] == ':' {
                i += 1;
            }
            while i < len && is_ident_char(chars[i]) {
                i += 1;
            }
            // Skip function arguments like :not(...)
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
            continue;
        }

        // Skip attribute selectors [attr]
        if chars[i] == '[' {
            while i < len && chars[i] != ']' {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            continue;
        }

        // Skip * (universal selector)
        if chars[i] == '*' {
            i += 1;
            continue;
        }

        i += 1;
    }
    false
}

fn is_type_selector_start(chars: &[char], i: usize) -> bool {
    let ch = chars[i];
    if !ch.is_ascii_alphabetic() && ch.is_ascii() {
        return false;
    }
    // Must not be preceded by '.', '#', or ':' (which would mean it's part of a class/id/pseudo)
    if i > 0 {
        let prev = chars[i - 1];
        if prev == '.' || prev == '#' || prev == ':' {
            return false;
        }
    }
    true
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || !ch.is_ascii()
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
    fn reports_type_with_class() {
        let d = SelectorNoQualifyingType.check(&style_with_selector("a.foo"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("a.foo"));
    }

    #[test]
    fn reports_type_with_id() {
        let d = SelectorNoQualifyingType.check(&style_with_selector("div#bar"), &ctx());
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn allows_class_only() {
        let d = SelectorNoQualifyingType.check(&style_with_selector(".foo"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_type_only() {
        let d = SelectorNoQualifyingType.check(&style_with_selector("div"), &ctx());
        assert!(d.is_empty());
    }
}
