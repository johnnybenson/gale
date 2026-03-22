use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require a newline or disallow whitespace after the commas of selector lists.
///
/// Equivalent to `@stylistic/selector-list-comma-newline-after`.
pub struct StylisticSelectorListCommaNewlineAfter;

impl Rule for StylisticSelectorListCommaNewlineAfter {
    fn name(&self) -> &'static str {
        "@stylistic/selector-list-comma-newline-after"
    }

    fn description(&self) -> &'static str {
        "Require a newline or disallow whitespace after the commas of selector lists"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let option = ctx.primary_option_str().unwrap_or("always-multi-line");
        let selector = &rule.selector;

        // Check if this is a selector list (contains commas)
        if !selector.contains(',') {
            return vec![];
        }

        let is_multi_line = selector.contains('\n');

        let mut diags = Vec::new();
        let sel_offset = rule.span.offset;

        // Find commas in the source text of the selector for accurate spans
        let sel_source = if sel_offset + selector.len() <= ctx.source.len() {
            &ctx.source[sel_offset..sel_offset + selector.len()]
        } else {
            selector.as_str()
        };

        let bytes = sel_source.as_bytes();
        let mut paren_depth = 0i32;
        let mut in_interpolation = 0i32;
        for (i, &b) in bytes.iter().enumerate() {
            // Track SCSS interpolation #{...}
            if b == b'#' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                in_interpolation += 1;
            }
            if b == b'{' && i > 0 && bytes[i - 1] != b'#' {
                // regular brace, not interpolation
            } else if b == b'{' && i > 0 && bytes[i - 1] == b'#' {
                // already counted above
            }
            if b == b'}' && in_interpolation > 0 {
                in_interpolation -= 1;
            }
            if b == b'(' {
                paren_depth += 1;
            }
            if b == b')' && paren_depth > 0 {
                paren_depth -= 1;
            }
            // Only check commas at the top level of the selector
            // (not inside interpolation or function calls)
            if b == b',' && paren_depth == 0 && in_interpolation == 0 {
                let after_comma = i + 1;
                let next_char = if after_comma < bytes.len() {
                    Some(bytes[after_comma] as char)
                } else {
                    None
                };

                // SCSS line comment after comma counts as having a newline
                let has_scss_comment = after_comma + 1 < bytes.len()
                    && {
                        let mut skip = after_comma;
                        while skip < bytes.len()
                            && (bytes[skip] == b' ' || bytes[skip] == b'\t')
                        {
                            skip += 1;
                        }
                        skip + 1 < bytes.len()
                            && bytes[skip] == b'/'
                            && bytes[skip + 1] == b'/'
                    };
                let has_newline_after = has_scss_comment
                    || next_char == Some('\n')
                    || (next_char == Some('\r')
                        && after_comma + 1 < bytes.len()
                        && bytes[after_comma + 1] == b'\n');

                let violation = match option {
                    "always" => !has_newline_after,
                    "never" => next_char.is_some_and(|c| c == '\n' || c == '\r'),
                    "always-multi-line" => is_multi_line && !has_newline_after,
                    _ => false,
                };

                if violation {
                    let msg = match option {
                        "always" | "always-multi-line" => "Expected newline after \",\"",
                        "never" => "Unexpected newline after \",\"",
                        _ => continue,
                    };
                    diags.push(
                        Diagnostic::new(self.name(), msg)
                            .severity(self.default_severity())
                            .span(Span::new(sel_offset + i, 1)),
                    );
                }
            }
        }
        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Span as ParserSpan, StyleRule, Syntax};

    fn ctx_with_source(source: &str) -> RuleContext<'_> {
        RuleContext {
            file_path: "t.css",
            source,
            syntax: Syntax::Css,
            options: None,
        }
    }

    fn style_with_selector(sel: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: sel.to_string(),
            declarations: vec![],
span: ParserSpan::new(0, sel.len()),
            ..Default::default()
})
    }

    #[test]
    fn reports_missing_newline_after_comma_multiline() {
        let rule = StylisticSelectorListCommaNewlineAfter;
        let sel = "a,\nb, c";
        let ctx = ctx_with_source(sel);
        let d = rule.check(&style_with_selector(sel), &ctx);
        // The second comma (b, c) has a space instead of newline
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("newline"));
    }

    #[test]
    fn allows_newline_after_all_commas() {
        let rule = StylisticSelectorListCommaNewlineAfter;
        let sel = "a,\nb,\nc";
        let ctx = ctx_with_source(sel);
        let d = rule.check(&style_with_selector(sel), &ctx);
        assert!(d.is_empty());
    }

    #[test]
    fn allows_single_line_with_default() {
        let rule = StylisticSelectorListCommaNewlineAfter;
        let sel = "a, b, c";
        let ctx = ctx_with_source(sel);
        let d = rule.check(&style_with_selector(sel), &ctx);
        // Default is "always-multi-line", single line is fine
        assert!(d.is_empty());
    }
}
