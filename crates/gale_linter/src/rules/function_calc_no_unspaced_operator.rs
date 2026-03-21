use gale_css_parser::{CssNode, Syntax};
use gale_diagnostics::{Diagnostic, Severity, Span};
use regex::Regex;
use std::sync::LazyLock;

use crate::rule::{Rule, RuleContext};

/// Reports when `+` or `-` operators inside `calc()` expressions are not
/// surrounded by spaces.
///
/// Per the CSS spec, `+` and `-` inside `calc()` must have whitespace on both
/// sides. `*` and `/` do not require spaces.
///
/// Equivalent to Stylelint's `function-calc-no-unspaced-operator` rule.
pub struct FunctionCalcNoUnspacedOperator;

/// Regex to find `calc(...)` expressions, handling nested parentheses is done
/// manually after finding the start.
static CALC_START: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)calc\(").unwrap());

/// Extract the content inside a `calc(` starting at the given position.
/// Returns the substring inside the outermost `calc(...)`.
fn extract_calc_body(s: &str, start: usize) -> Option<&str> {
    let mut depth = 1;
    let inner_start = start;
    let bytes = s.as_bytes();
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[inner_start..i]);
                }
            }
            _ => {}
        }
        i += 1;
    }
    // Unclosed calc, return what we have.
    Some(&s[inner_start..])
}

/// Describes which side(s) of an operator are missing a space.
#[derive(Debug, Clone, Copy, PartialEq)]
enum UnspacedSide {
    Before,
    After,
}

/// Check a calc body for unspaced `+` or `-` operators.
/// Rules: `+` and `-` must be preceded and followed by whitespace.
/// Exception: `-` at the very start (unary minus) or after `(` is fine.
/// Only checks at the top-level nesting depth (depth 0); operators inside
/// nested function calls like `var(--prop)` or `min(…)` are ignored.
///
/// Returns a list of violations (one per missing-space side), matching
/// Stylelint's behaviour of emitting separate diagnostics for "before"
/// and "after".
fn find_unspaced_operators(body: &str) -> Vec<(usize, char, UnspacedSide)> {
    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut depth = 0i32; // parenthesis nesting depth
    let mut results = Vec::new();

    for i in 0..len {
        let ch = bytes[i];

        if ch == b'(' {
            depth += 1;
            continue;
        }
        if ch == b')' {
            depth -= 1;
            continue;
        }

        // Only check operators at the top level of the calc body.
        if depth > 0 {
            continue;
        }

        if ch == b'+' || ch == b'-' {
            // Check if preceded by 'e' or 'E' (scientific notation)
            if i > 0 && (bytes[i - 1] == b'e' || bytes[i - 1] == b'E') {
                continue;
            }

            // Check for whitespace before (space, tab, newline)
            let has_space_before = i > 0 && bytes[i - 1].is_ascii_whitespace();
            // Check for whitespace after
            let has_space_after = i + 1 < len && bytes[i + 1].is_ascii_whitespace();

            // Skip unary cases: after `(` or at start
            if i == 0 {
                continue;
            }
            if bytes[i - 1] == b'(' {
                continue;
            }

            // Skip unary `-` or `+` after another operator (`*`, `/`, `+`, `-`).
            // This handles patterns like `expr * -1` where `-` is unary.
            {
                let mut j = i;
                while j > 0 && bytes[j - 1].is_ascii_whitespace() {
                    j -= 1;
                }
                if j > 0
                    && (bytes[j - 1] == b'*'
                        || bytes[j - 1] == b'/'
                        || bytes[j - 1] == b'+'
                        || bytes[j - 1] == b'-'
                        || bytes[j - 1] == b',')
                {
                    continue;
                }
            }

            let op_char = ch as char;

            // Emit separate diagnostics for each missing side, matching Stylelint.
            if !has_space_before {
                results.push((i, op_char, UnspacedSide::Before));
            }
            if !has_space_after {
                results.push((i, op_char, UnspacedSide::After));
            }
        }
    }

    results
}

impl Rule for FunctionCalcNoUnspacedOperator {
    fn name(&self) -> &'static str {
        "function-calc-no-unspaced-operator"
    }

    fn description(&self) -> &'static str {
        "Disallow unspaced operators within calc functions"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, context: &RuleContext) -> Vec<Diagnostic> {
        let style = match node {
            CssNode::Style(s) => s,
            _ => return vec![],
        };

        let mut diagnostics = Vec::new();

        for decl in &style.declarations {
            let value = &decl.value;

            let is_scss = matches!(context.syntax, Syntax::Scss | Syntax::Less | Syntax::Sass);

            for m in CALC_START.find_iter(value) {
                let body_start = m.end();
                if let Some(body) = extract_calc_body(value, body_start) {
                    // In SCSS/Less/Sass, skip calc() bodies that contain
                    // interpolation (`#{...}`) or SCSS variables (`$var`).
                    // These will be compiled to valid CSS by the preprocessor.
                    if is_scss && (body.contains("#{") || body.contains('$')) {
                        continue;
                    }

                    let violations = find_unspaced_operators(body);
                    for (_pos, op, side) in &violations {
                        let msg = match side {
                            UnspacedSide::Before => {
                                format!(
                                    "Expected a space before the '{op}' operator in calc function"
                                )
                            }
                            UnspacedSide::After => {
                                format!(
                                    "Expected a space after the '{op}' operator in calc function"
                                )
                            }
                        };
                        diagnostics.push(
                            Diagnostic::new(self.name(), msg)
                                .severity(self.default_severity())
                                .span(Span::new(decl.span.offset, decl.span.length)),
                        );
                    }
                }
            }
        }

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn make_context() -> RuleContext<'static> {
        RuleContext {
            file_path: "test.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        }
    }

    #[test]
    fn reports_unspaced_plus_in_calc_both_sides() {
        let rule = FunctionCalcNoUnspacedOperator;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "width".to_string(),
                value: "calc(100%+20px)".to_string(),
                span: ParserSpan::new(4, 22),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 28),
        });
        let diags = rule.check(&node, &make_context());
        // Both sides unspaced → 2 diagnostics (before + after)
        assert_eq!(diags.len(), 2);
        assert!(diags[0].message.contains("before"));
        assert!(diags[1].message.contains("after"));
    }

    #[test]
    fn reports_unspaced_plus_in_calc_one_side() {
        let rule = FunctionCalcNoUnspacedOperator;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "width".to_string(),
                value: "calc(100% +20px)".to_string(),
                span: ParserSpan::new(4, 23),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 29),
        });
        let diags = rule.check(&node, &make_context());
        // Only missing space after → 1 diagnostic
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("after"));
    }

    #[test]
    fn reports_unspaced_minus_in_calc_both_sides() {
        let rule = FunctionCalcNoUnspacedOperator;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "width".to_string(),
                value: "calc(100%-20px)".to_string(),
                span: ParserSpan::new(4, 22),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 28),
        });
        let diags = rule.check(&node, &make_context());
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn ignores_properly_spaced_calc() {
        let rule = FunctionCalcNoUnspacedOperator;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "width".to_string(),
                value: "calc(100% - 20px)".to_string(),
                span: ParserSpan::new(4, 24),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 30),
        });
        let diags = rule.check(&node, &make_context());
        assert!(diags.is_empty());
    }

    #[test]
    fn skips_scss_interpolation_in_calc() {
        let rule = FunctionCalcNoUnspacedOperator;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "width".to_string(),
                value: "calc(100%-#{$gap})".to_string(),
                span: ParserSpan::new(4, 24),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 30),
        });
        let ctx = RuleContext {
            file_path: "test.scss",
            source: "",
            syntax: Syntax::Scss,
            options: None,
        };
        let diags = rule.check(&node, &ctx);
        assert!(diags.is_empty());
    }

    #[test]
    fn ignores_unary_minus() {
        let rule = FunctionCalcNoUnspacedOperator;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "width".to_string(),
                value: "calc(-20px + 100%)".to_string(),
                span: ParserSpan::new(4, 25),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 31),
        });
        let diags = rule.check(&node, &make_context());
        assert!(diags.is_empty());
    }
}
