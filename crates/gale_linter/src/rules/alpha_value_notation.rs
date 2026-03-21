use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Prefer percentage notation for alpha values in `rgba()` and `hsla()`.
///
/// For example, `rgba(0,0,0,0.5)` should use `rgba(0,0,0,50%)` instead.
///
/// Equivalent to Stylelint's `alpha-value-notation` rule with "percentage" option.
/// Detection-only (no autofix).
pub struct AlphaValueNotation;

impl Rule for AlphaValueNotation {
    fn name(&self) -> &'static str {
        "alpha-value-notation"
    }

    fn description(&self) -> &'static str {
        "Prefer percentage notation for alpha values"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let mut diags = Vec::new();
        for decl in &rule.declarations {
            let decl_start = decl.span.offset;
            let decl_end = decl_start + decl.span.length;
            let search_area = if decl_end <= ctx.source.len() && decl_start < decl_end {
                &ctx.source[decl_start..decl_end]
            } else {
                &decl.value
            };

            for (rel_offset, func_name) in find_alpha_functions(search_area) {
                let abs_offset = if decl_end <= ctx.source.len() && decl_start < decl_end {
                    decl_start + rel_offset
                } else {
                    decl_start
                };
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected percentage notation for alpha value in {func_name}()"
                        ),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(abs_offset, 1)),
                );
            }
        }
        diags
    }
}

/// Find `rgba(` or `hsla(` calls where the 4th argument is a decimal (not percentage).
/// Returns (byte_offset_of_alpha_arg, function_name).
fn find_alpha_functions(value: &str) -> Vec<(usize, String)> {
    let mut results = Vec::new();
    let lower = value.to_ascii_lowercase();

    for func in &["rgba", "hsla"] {
        let pattern = format!("{func}(");
        let mut search_from = 0;
        while let Some(pos) = lower[search_from..].find(&pattern) {
            let abs_pos = search_from + pos;
            let args_start = abs_pos + pattern.len();
            if let Some(close) = value[args_start..].find(')') {
                let args = &value[args_start..args_start + close];
                // Split by comma for legacy syntax, or by space/slash for modern syntax.
                let alpha = if args.contains(',') {
                    // Legacy: rgba(r, g, b, a)
                    let parts: Vec<&str> = args.split(',').collect();
                    if parts.len() == 4 {
                        Some((parts[3].trim(), args_start + find_nth_arg_offset(args, 3)))
                    } else {
                        None
                    }
                } else if args.contains('/') {
                    // Modern: rgba(r g b / a)
                    let parts: Vec<&str> = args.split('/').collect();
                    if parts.len() == 2 {
                        Some((
                            parts[1].trim(),
                            args_start + args.find('/').unwrap() + 1
                                + parts[1].len()
                                - parts[1].trim_start().len(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some((alpha_val, alpha_offset)) = alpha {
                    // Check if the alpha value is a decimal number (not a percentage).
                    let trimmed = alpha_val.trim();
                    if !trimmed.ends_with('%') && is_decimal_number(trimmed) {
                        results.push((alpha_offset, func.to_string()));
                    }
                }
            }
            search_from = abs_pos + 1;
        }
    }
    results
}

/// Find the byte offset of the nth comma-separated argument within the string.
fn find_nth_arg_offset(args: &str, n: usize) -> usize {
    let mut count = 0;
    for (i, b) in args.bytes().enumerate() {
        if b == b',' {
            count += 1;
            if count == n {
                // Skip the comma and any whitespace
                let rest = &args[i + 1..];
                let ws = rest.len() - rest.trim_start().len();
                return i + 1 + ws;
            }
        }
    }
    0
}

fn is_decimal_number(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut has_digit = false;
    let mut has_dot = false;
    for (i, c) in s.chars().enumerate() {
        if c == '.' {
            if has_dot {
                return false;
            }
            has_dot = true;
        } else if c.is_ascii_digit() {
            has_digit = true;
        } else if c == '-' || c == '+' {
            if i != 0 {
                return false;
            }
        } else {
            return false;
        }
    }
    has_digit
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
        }
    }

    fn style_with_value(val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "color".to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_decimal_alpha_in_rgba() {
        let d = AlphaValueNotation.check(&style_with_value("rgba(0, 0, 0, 0.5)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("rgba()"));
    }

    #[test]
    fn allows_percentage_alpha() {
        let d = AlphaValueNotation.check(&style_with_value("rgba(0, 0, 0, 50%)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn reports_decimal_alpha_in_hsla() {
        let d = AlphaValueNotation.check(&style_with_value("hsla(0, 100%, 50%, 0.8)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("hsla()"));
    }

    #[test]
    fn allows_no_alpha_function() {
        let d = AlphaValueNotation.check(&style_with_value("rgb(0, 0, 0)"), &ctx());
        assert!(d.is_empty());
    }
}
