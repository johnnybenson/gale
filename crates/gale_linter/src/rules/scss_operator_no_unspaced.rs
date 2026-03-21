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
            let slash_is_separator = is_shorthand_with_slash(&decl.property);
            check_scss_operators(
                &decl.value,
                decl.span.offset,
                slash_is_separator,
                self,
                &mut diagnostics,
            );
        }

        diagnostics
    }
}

/// Properties where `/` acts as a CSS value separator rather than a division
/// operator. In these shorthand properties the `/` separates different
/// sub-values (e.g., `font: 14px/1.5`, `border-radius: 10px / 5px`,
/// `grid: auto-flow / 1fr 1fr`).
fn is_shorthand_with_slash(property: &str) -> bool {
    let prop = property.to_ascii_lowercase();
    // Strip any vendor prefix (e.g., -webkit-border-radius).
    let prop = prop
        .strip_prefix("-webkit-")
        .or_else(|| prop.strip_prefix("-moz-"))
        .or_else(|| prop.strip_prefix("-ms-"))
        .or_else(|| prop.strip_prefix("-o-"))
        .unwrap_or(&prop);

    matches!(
        prop,
        "font"
            | "border-radius"
            | "background"
            | "background-size"
            | "grid"
            | "grid-area"
            | "grid-column"
            | "grid-row"
            | "grid-template"
            | "grid-template-columns"
            | "grid-template-rows"
            | "list-style"
            | "mask"
            | "mask-border"
    )
}

fn check_scss_operators(
    value: &str,
    base_offset: usize,
    slash_is_separator: bool,
    rule: &ScssOperatorNoUnspaced,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let bytes = value.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut paren_depth: i32 = 0;
    let mut in_calc = false;
    let mut calc_paren_depth: i32 = 0;
    let mut in_url = false;
    let mut url_paren_depth: i32 = 0;

    while i < len {
        let ch = bytes[i];

        // Skip SCSS single-line comments: `// ...` until end of line.
        if ch == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

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

        // Track url() depth — skip `/` inside url() (path separators).
        if i + 4 <= len && value[i..].to_ascii_lowercase().starts_with("url(") {
            in_url = true;
            url_paren_depth = paren_depth + 1;
            i += 4;
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
            if in_url && paren_depth == url_paren_depth {
                in_url = false;
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

        // Skip everything inside url() — slashes are path separators.
        if in_url && paren_depth >= url_paren_depth {
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

            // Skip `/` in shorthand properties where it's a CSS value
            // separator, not a division operator (e.g., `font: 14px/1.5`,
            // `border-radius: 10px / 5px`, `grid: auto-flow / 1fr`).
            if ch == b'/' && slash_is_separator {
                i += 1;
                continue;
            }

            // Skip unary operators.
            if (ch == b'+' || ch == b'-') && is_unary(bytes, i) {
                i += 1;
                continue;
            }

            // Skip negative signs: `-` that acts as a negative prefix rather
            // than a subtraction operator. A `-` is a negative sign when:
            // - It immediately precedes a digit, `$`, or `.` (no space after)
            // - The preceding non-whitespace character is `,`, `(`, `:`, or
            //   start of value (i.e., NOT a "value token" like a number,
            //   variable, closing paren, or identifier).
            if ch == b'-' && is_negative_sign(bytes, i) {
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

/// Check if `-` at position `i` is a negative sign (not a subtraction operator).
///
/// A `-` is a negative sign when it immediately precedes a digit or `.` (a CSS
/// numeric literal) with a space before it. This covers cases like:
/// - `box-shadow: 0 -2px red` — `-2px` is a negative length value
/// - `margin: 0 -10px` — negative value in a space-separated list
///
/// A `-` before `$` (SCSS variable) is treated as subtraction, since
/// `-$var` in a math context like `$a -$b` is typically subtraction.
///
/// When there is NO space before `-`, it falls through to the normal
/// operator spacing check (and the existing hyphen/identifier logic).
fn is_negative_sign(bytes: &[u8], i: usize) -> bool {
    let len = bytes.len();

    // Must have a space before the `-`.
    if i == 0 || !bytes[i - 1].is_ascii_whitespace() {
        return false;
    }

    // Must immediately precede a digit or `.` (CSS numeric value), not `$`.
    // `-$var` after a value token is subtraction in SCSS.
    if i + 1 >= len {
        return false;
    }
    let next = bytes[i + 1];
    if !(next.is_ascii_digit() || next == b'.') {
        return false;
    }

    true
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

    #[test]
    fn skips_border_radius_shorthand_slash() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("border-radius", "10px 5px / 20px 15px");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "border-radius slash is a separator, not division");
    }

    #[test]
    fn skips_border_radius_unspaced_slash() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("border-radius", "10px/5px");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "border-radius unspaced slash is a separator");
    }

    #[test]
    fn skips_grid_shorthand_slash() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("grid", "auto-flow / 1fr 1fr 1fr");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "grid slash is a separator");
    }

    #[test]
    fn skips_grid_column_slash() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("grid-column", "1 / 3");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "grid-column slash is a separator");
    }

    #[test]
    fn skips_grid_row_slash() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("grid-row", "1/3");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "grid-row unspaced slash is a separator");
    }

    #[test]
    fn skips_grid_area_slash() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("grid-area", "1 / 1 / 3 / 3");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "grid-area slashes are separators");
    }

    #[test]
    fn skips_grid_template_slash() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("grid-template", "'a a' 100px / 1fr 1fr");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "grid-template slash is a separator");
    }

    #[test]
    fn skips_background_shorthand_slash() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("background", "url(img.png) center/cover no-repeat");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "background slash is a separator");
    }

    #[test]
    fn skips_list_style_shorthand_slash() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("list-style", "disc outside/inside");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "list-style slash is a separator");
    }

    #[test]
    fn skips_slash_in_url() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("background-image", "url(https://example.com/path/to/image.png)");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "slashes inside url() should be ignored");
    }

    #[test]
    fn skips_slash_in_url_with_quotes() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("background-image", "url('https://example.com/path/to/image.png')");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "slashes inside url() with quotes should be ignored");
    }

    #[test]
    fn skips_vendor_prefixed_border_radius() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("-webkit-border-radius", "10px/5px");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "vendor-prefixed border-radius slash is a separator");
    }

    #[test]
    fn still_reports_unspaced_slash_in_math() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("width", "$a/2");
        let diags = rule.check(&node, &scss_context());
        assert!(!diags.is_empty(), "division operator should still be flagged");
    }

    #[test]
    fn skips_inline_comment() {
        let rule = ScssOperatorNoUnspaced;
        let node = make_node("box-shadow", "0 0 0 3px blue, // comment");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "inline comment // should not be flagged as unspaced /");
    }

    #[test]
    fn skips_negative_value_after_space() {
        let rule = ScssOperatorNoUnspaced;
        // margin: -10px (unary at start)
        let node = make_node("margin", "-10px");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "-10px at start of value is a negative sign");

        // top: -2px
        let node = make_node("top", "-2px");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "-2px at start of value is a negative sign");

        // box-shadow: 0 -2px red
        let node = make_node("box-shadow", "0 -2px red");
        let diags = rule.check(&node, &scss_context());
        assert!(diags.is_empty(), "-2px after space is a negative value, not subtraction");
    }

    #[test]
    fn flags_negative_variable_as_subtraction() {
        let rule = ScssOperatorNoUnspaced;
        // `0 -$offset` — `-` before `$` is treated as subtraction in SCSS.
        let node = make_node("box-shadow", "0 -$offset red");
        let diags = rule.check(&node, &scss_context());
        assert!(!diags.is_empty(), "-$var after a value is subtraction in SCSS");
    }

    #[test]
    fn still_reports_unspaced_subtraction() {
        let rule = ScssOperatorNoUnspaced;
        // $a-$b: no spaces around `-`, should flag.
        let node = make_node("width", "$a-$b");
        let diags = rule.check(&node, &scss_context());
        assert!(!diags.is_empty(), "$a-$b is unspaced subtraction and should be flagged");

        // $a -$b: space before but not after `-`, still subtraction since
        // previous non-whitespace is a value token (`a`).
        let node = make_node("width", "$a -$b");
        let diags = rule.check(&node, &scss_context());
        assert!(!diags.is_empty(), "$a -$b should flag: - is subtraction after a value token");
    }

    #[test]
    fn still_reports_unspaced_operators_in_shorthand() {
        let rule = ScssOperatorNoUnspaced;
        // Even in border-radius, + and * should still be checked.
        let node = make_node("border-radius", "$a+$b");
        let diags = rule.check(&node, &scss_context());
        assert!(!diags.is_empty(), "non-slash operators should still be flagged in shorthands");
    }
}
