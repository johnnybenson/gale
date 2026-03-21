use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Specify the minimum number of milliseconds for time values.
///
/// Equivalent to Stylelint's `time-min-milliseconds` rule.
/// Default minimum: 100ms.
pub struct TimeMinMilliseconds;

const DEFAULT_MIN: f64 = 100.0;

impl Rule for TimeMinMilliseconds {
    fn name(&self) -> &'static str {
        "time-min-milliseconds"
    }

    fn description(&self) -> &'static str {
        "Specify the minimum number of milliseconds for time values"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        let min = ctx
            .options
            .and_then(|v| v.as_f64())
            .unwrap_or(DEFAULT_MIN);

        let mut diags = Vec::new();
        for decl in &rule.declarations {
            for time_ms in extract_time_values_ms(&decl.value) {
                // Skip 0 — zero duration is intentional and should not be flagged
                if time_ms == 0.0 {
                    continue;
                }
                if time_ms < min {
                    diags.push(
                        Diagnostic::new(
                            self.name(),
                            format!(
                                "Expected a minimum of {min}ms, found {time_ms}ms"
                            ),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(decl.span.offset, decl.span.length)),
                    );
                }
            }
        }
        diags
    }
}

/// Extract time values from a declaration value and convert to milliseconds.
/// Recognises values like `200ms`, `0.5s`, `10ms`.
fn extract_time_values_ms(value: &str) -> Vec<f64> {
    let mut times = Vec::new();
    let lower = value.to_ascii_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Look for start of a number (digit or dot)
        if chars[i].is_ascii_digit() || (chars[i] == '.' && i + 1 < len && chars[i + 1].is_ascii_digit()) {
            let start = i;
            // Consume digits and dot
            while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let num_str: String = chars[start..i].iter().collect();
            if let Ok(num) = num_str.parse::<f64>() {
                // Check for unit
                if i + 2 <= len && chars[i] == 'm' && chars[i + 1] == 's' {
                    // Make sure 'ms' is not followed by an alpha char (e.g. not part of a word)
                    let after_unit = i + 2;
                    if after_unit >= len || !chars[after_unit].is_ascii_alphabetic() {
                        times.push(num);
                        i += 2;
                        continue;
                    }
                } else if i < len && chars[i] == 's' {
                    let after_unit = i + 1;
                    if after_unit >= len || !chars[after_unit].is_ascii_alphabetic() {
                        times.push(num * 1000.0);
                        i += 1;
                        continue;
                    }
                }
            }
        } else {
            i += 1;
        }
    }

    times
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

    fn ctx_with_options(options: serde_json::Value) -> RuleContext<'static> {
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: Some(Box::leak(Box::new(options))),
        }
    }

    fn style_with_decl(prop: &str, val: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: prop.to_string(),
                value: val.to_string(),
                span: ParserSpan::new(0, prop.len() + val.len() + 2),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_time_below_minimum() {
        let d = TimeMinMilliseconds.check(&style_with_decl("transition", "all 50ms"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("50ms"));
    }

    #[test]
    fn allows_time_above_minimum() {
        let d = TimeMinMilliseconds.check(&style_with_decl("transition", "all 200ms"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn converts_seconds_to_ms() {
        // 0.01s = 10ms, should flag with default minimum 100ms
        let d = TimeMinMilliseconds.check(&style_with_decl("transition", "all 0.01s"), &ctx());
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn allows_zero() {
        let d = TimeMinMilliseconds.check(&style_with_decl("transition", "all 0ms"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn respects_configured_min() {
        let ctx = ctx_with_options(serde_json::json!(50));
        let d = TimeMinMilliseconds.check(&style_with_decl("transition", "all 30ms"), &ctx);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn rule_name_is_correct() {
        assert_eq!(TimeMinMilliseconds.name(), "time-min-milliseconds");
    }
}
