use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Prefer modern color function notation (space-separated) over legacy (comma-separated).
///
/// Equivalent to Stylelint's `color-function-notation` rule with "modern" option.
/// Detects comma-separated arguments in rgb/rgba/hsl/hsla. Detection-only.
pub struct ColorFunctionNotation;

const COLOR_FUNCTIONS: &[&str] = &["rgb(", "rgba(", "hsl(", "hsla("];

/// Find the position of the matching closing paren, handling nesting.
/// Returns the offset relative to the start of `s` (which should begin
/// right after the opening paren).
fn find_matching_paren(s: &str) -> Option<usize> {
    let mut depth: i32 = 1;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

impl Rule for ColorFunctionNotation {
    fn name(&self) -> &'static str {
        "color-function-notation"
    }

    fn description(&self) -> &'static str {
        "Specify modern or legacy notation for color functions"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        // Read secondary options for `ignore: ["with-var-inside"]`
        let ignore_with_var = ctx
            .secondary_options()
            .or(ctx.options)
            .and_then(|v| v.get("ignore"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .any(|item| item.as_str() == Some("with-var-inside"))
            })
            .unwrap_or(false);

        let mut diags = Vec::new();
        for decl in &rule.declarations {
            let lower = decl.value.to_ascii_lowercase();

            // Pre-compute: find where the value starts within the declaration source.
            // This lets us map a byte-position in `decl.value` → byte-position in source.
            let decl_src_end = (decl.span.offset + decl.span.length).min(ctx.source.len());
            let decl_src = &ctx.source[decl.span.offset..decl_src_end];
            let decl_src_lower = decl_src.to_ascii_lowercase();
            // The value may not match exactly (multi-line whitespace differences), so fall back to 0.
            let val_start_in_decl = decl_src_lower.find(lower.as_str()).unwrap_or(0);

            for &func in COLOR_FUNCTIONS {
                let mut search_from = 0;
                while let Some(pos) = lower[search_from..].find(func) {
                    let abs_pos = search_from + pos;
                    let args_start = abs_pos + func.len();
                    if let Some(close) = find_matching_paren(&lower[args_start..]) {
                        let args = &decl.value[args_start..args_start + close];
                        if args.contains(',') {
                            // Skip if ignore: ["with-var-inside"] and args contain var(
                            if ignore_with_var && args.to_ascii_lowercase().contains("var(") {
                                search_from = abs_pos + 1;
                                continue;
                            }
                            let fn_name = &func[..func.len() - 1]; // strip trailing '('
                            // Use abs_pos (position in value) to locate this specific occurrence.
                            let fn_off = val_start_in_decl + abs_pos;
                            diags.push(
                                Diagnostic::new(
                                    self.name(),
                                    "Expected modern color-function notation".to_string(),
                                )
                                .severity(self.default_severity())
                                .span(Span::new(decl.span.offset + fn_off, fn_name.len())),
                            );
                        }
                    }
                    search_from = abs_pos + 1;
                }
            }
        }
        diags
    }
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

    fn style_with_value(value: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "color".to_string(),
                value: value.to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            span: ParserSpan::new(0, 0),
            ..Default::default()
        })
    }

    #[test]
    fn reports_legacy_rgb() {
        let d = ColorFunctionNotation.check(&style_with_value("rgb(0, 0, 0)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("color-function notation"));
    }

    #[test]
    fn allows_modern_rgb() {
        let d = ColorFunctionNotation.check(&style_with_value("rgb(0 0 0)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn reports_legacy_hsl() {
        let d = ColorFunctionNotation.check(&style_with_value("hsl(0, 100%, 50%)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("color-function notation"));
    }

    #[test]
    fn allows_modern_hsl() {
        let d = ColorFunctionNotation.check(&style_with_value("hsl(0 100% 50%)"), &ctx());
        assert!(d.is_empty());
    }
}
