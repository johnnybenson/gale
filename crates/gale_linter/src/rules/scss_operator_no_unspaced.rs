use gale_css_parser::{CssNode, Syntax};
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow unspaced operators in SCSS expressions.
///
/// Checks `+`, `-`, `*`, `/` operators in declaration values.
/// Skips operators inside `calc()` (handled by `function-calc-no-unspaced-operator`).
/// Skips negative numbers, unary operators, `/` in `font` shorthand, and
/// string literals.
///
/// Equivalent to `scss/operator-no-unspaced`.
pub struct ScssOperatorNoUnspaced;

impl Rule for ScssOperatorNoUnspaced {
    fn name(&self) -> &'static str {
        "scss/operator-no-unspaced"
    }

    fn description(&self) -> &'static str {
        "Disallow unspaced Sass operators"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, context: &RuleContext) -> Vec<Diagnostic> {
        // Only applies to SCSS/Less/Sass files.
        if !matches!(context.syntax, Syntax::Scss | Syntax::Less | Syntax::Sass) {
            return vec![];
        }

        let style = match node {
            CssNode::Style(s) => s,
            _ => return vec![],
        };

        let mut diagnostics = Vec::new();

        for decl in &style.declarations {
            let is_font = decl.property.eq_ignore_ascii_case("font");
            check_scss_operators(
                &decl.value,
                decl.span.offset,
                is_font,
                self,
                &mut diagnostics,
            );
        }

        diagnostics
    }
}

fn check_scss_operators(
    value: &str,
    base_offset: usize,
    is_font_property: bool,
    rule: &ScssOperatorNoUnspaced,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let bytes = value.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut paren_depth: i32 = 0;
    let mut in_calc = false;
    let mut calc_paren_depth: i32 = 0;

    while i < len {
        let ch = bytes[i];

        // Skip string literals.
        if ch == b'"' || ch == b'\'' {
            let quote = ch;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Skip SCSS interpolation #{...}
        if ch == b'#' && i + 1 < len && bytes[i + 1] == b'{' {
            i += 2;
            let mut depth = 1i32;
            while i < len && depth > 0 {
                if bytes[i] == b'{' {
                    depth += 1;
                } else if bytes[i] == b'}' {
                    depth -= 1;
                }
                i += 1;
            }
            continue;
        }

        // Track calc() depth — skip operators inside calc.
        if i + 5 <= len && value[i..].to_ascii_lowercase().starts_with("calc(") {
            in_calc = true;
            calc_paren_depth = paren_depth + 1;
            i += 5;
            paren_depth += 1;
            continue;
        }

        if ch == b'(' {
            paren_depth += 1;
            i += 1;
            continue;
        }
        if ch == b')' {
            if in_calc && paren_depth == calc_paren_depth {
                in_calc = false;
            }
            paren_depth -= 1;
            i += 1;
            continue;
        }

        // Skip operators inside calc() — those are handled by
        // function-calc-no-unspaced-operator.
        if in_calc && paren_depth >= calc_paren_depth {
            i += 1;
            continue;
        }

        // Check for operators: +, -, *, /
        if ch == b'+' || ch == b'-' || ch == b'*' || ch == b'/' {
            // Scientific notation (e.g., 1e+2, 1E-3)
            if (ch == b'+' || ch == b'-') && i > 0 && (bytes[i - 1] == b'e' || bytes[i - 1] == b'E')
            {
                i += 1;
                continue;
            }

            // Skip `-` inside identifiers (e.g., `sans-serif`, `border-top`)
            // Also skip `-` after SCSS interpolation closing `}` (e.g.,
            // `#{$var}-suffix`) and before `#` for `prefix-#{$var}`.
            if ch == b'-' {
                let prev_is_ident = i > 0
                    && (bytes[i - 1].is_ascii_alphanumeric()
                        || bytes[i - 1] == b'_'
                        || bytes[i - 1] == b'}');
                let next_is_ident = i + 1 < len
                    && (bytes[i + 1].is_ascii_alphanumeric()
                        || bytes[i + 1] == b'_'
                        || bytes[i + 1] == b'#'
                        || bytes[i + 1] == b'-');
                if prev_is_ident && next_is_ident {
                    i += 1;
                    continue;
                }
            }

            // Skip `/` in font shorthand (e.g., `14px/1.5`)
            if ch == b'/' && is_font_property {
                i += 1;
                continue;
            }

            // Skip unary operators.
            if (ch == b'+' || ch == b'-') && is_unary(bytes, i) {
                i += 1;
                continue;
            }

            // Now check spacing.
            let has_space_before = i > 0 && bytes[i - 1].is_ascii_whitespace();
            let has_space_after = i + 1 < len && bytes[i + 1].is_ascii_whitespace();

            if !has_space_before || !has_space_after {
                diagnostics.push(
                    Diagnostic::new(
                        rule.name(),
                        format!("Expected spaces around operator '{}'", ch as char),
                    )
                    .severity(rule.default_severity())
                    .span(Span::new(base_offset + i, 1)),
                );
            }
        }

        i += 1;
    }
}

/// Check if an operator at position `i` is unary (not binary).
fn is_unary(bytes: &[u8], i: usize) -> bool {
    // At start of value → unary.
    if i == 0 {
        return true;
    }

    // After `(` → unary.
    if bytes[i - 1] == b'(' {
        return true;
    }

    // After `,` → unary.
    if bytes[i - 1] == b',' {
        return true;
    }

    // After `:` → unary (start of value part).
    if bytes[i - 1] == b':' {
        return true;
    }

    // Scan back past whitespace to find the preceding non-whitespace character.
    let mut j = i;
    while j > 0 && bytes[j - 1].is_ascii_whitespace() {
        j -= 1;
    }

    if j == 0 {
        return true; // only whitespace before → unary
    }

    let prev = bytes[j - 1];

    // After another operator → unary (e.g., `10px * -2`)
    if prev == b'+' || prev == b'-' || prev == b'*' || prev == b'/' || prev == b',' || prev == b'('
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule};

    fn scss_context() -> RuleContext<'static> {
        RuleContext {
            file_path: "test.scss",
            source: "",
            syntax: Syntax::Scss,
            options: None,
        }
    }

    fn css_context() -> RuleContext<'static> {
        RuleContext {
            file_path: "test.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        }
    }

    fn make_node(property: &str, value: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: property.to_string(),
                value: value.to_string(),
                span: ParserSpan::new(4, value.len()),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, value.len() + 20),
        })
    }

    #[test]
    fn skips_css_files() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("width", "10px+5px");
        let diags = rule.check(&node, &css_context());
        assert!(diags.is_empty());
    }

    #[test]
    fn reports_unspaced_plus() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("width", "$a+$b");
        let diags = rule.check(&node, &scss_context());
        assert!(!diags.is_empty());
    }

    #[test]
    fn reports_unspaced_multiply() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("width", "$a*2");
        let diags = rule.check(&node, &scss_context());
        assert!(!diags.is_empty());
    }

    #[test]
    fn allows_spaced_operators() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("width", "$a + $b");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty());
    }

    #[test]
    fn skips_unary_minus() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("margin", "-10px");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty());
    }

    #[test]
    fn skips_calc_operators() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("width", "calc(100%+20px)");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "calc operators should be skipped");
    }

    #[test]
    fn skips_font_shorthand_slash() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("font", "14px/1.5 sans-serif");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty());
    }

    #[test]
    fn skips_unary_after_operator() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("width", "$a * -1");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty());
    }
}
