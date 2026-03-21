use gale_css_parser::{CssNode, Syntax};
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require a newline or disallow whitespace after the closing brace of
/// `@if` statements that are NOT followed by `@else`.
///
/// Default: `"always-last-in-chain"` — require a newline after the closing
/// brace of the last `@if`/`@else if` in a chain (i.e. when not followed
/// by `@else`).
///
/// Equivalent to `scss/at-if-closing-brace-newline-after`.
pub struct ScssAtIfClosingBraceNewlineAfter;

impl Rule for ScssAtIfClosingBraceNewlineAfter {
    fn name(&self) -> &'static str {
        "scss/at-if-closing-brace-newline-after"
    }

    fn description(&self) -> &'static str {
        "Require or disallow a newline after the closing brace of @if statements"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, _nodes: &[CssNode], ctx: &RuleContext) -> Vec<Diagnostic> {
        if !matches!(ctx.syntax, Syntax::Scss | Syntax::Sass) {
            return vec![];
        }

        let option = ctx.primary_option_str().unwrap_or("always-last-in-chain");
        let source = ctx.source;
        let bytes = source.as_bytes();
        let len = bytes.len();
        let mut diagnostics = Vec::new();

        // Find `@if` occurrences and check what follows their closing `}`
        let mut i = 0;
        while i < len {
            // Look for `@if` or `@else if`
            if bytes[i] == b'@' && i + 2 < len {
                let rest = &source[i..];
                let is_if = rest.starts_with("@if ") || rest.starts_with("@if(");
                let is_else_if =
                    rest.starts_with("@else if ") || rest.starts_with("@else if(");

                if is_if || is_else_if {
                    // Find the matching closing `}`
                    if let Some(close_brace) = find_closing_brace(source, i) {
                        let after = close_brace + 1;
                        // Skip whitespace/spaces (not newlines) after `}`
                        let mut j = after;
                        while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                            j += 1;
                        }

                        // Check what follows
                        let followed_by_else =
                            j < len && source[j..].starts_with("@else");

                        match option {
                            "always-last-in-chain" => {
                                if !followed_by_else {
                                    // Must be followed by newline
                                    if j < len && bytes[j] != b'\n' && bytes[j] != b'\r' {
                                        diagnostics.push(
                                            Diagnostic::new(
                                                self.name(),
                                                "Expected newline after closing brace of @if"
                                                    .to_string(),
                                            )
                                            .severity(self.default_severity())
                                            .span(Span::new(close_brace, 1)),
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }

                        i = after;
                        continue;
                    }
                }
            }
            i += 1;
        }

        diagnostics
    }
}

/// Find the closing `}` that matches the first `{` found after `start`.
fn find_closing_brace(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = start;

    // Find opening brace
    while i < len && bytes[i] != b'{' {
        i += 1;
    }
    if i >= len {
        return None;
    }

    let mut depth = 1;
    i += 1;
    while i < len && depth > 0 {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        if depth > 0 {
            i += 1;
        }
    }

    if depth == 0 { Some(i) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::Syntax;

    fn scss_ctx_with_source(source: &str) -> RuleContext {
        RuleContext {
            file_path: "t.scss",
            source,
            syntax: Syntax::Scss,
            options: None,
        }
    }

    #[test]
    fn allows_newline_after_if() {
        let ctx = scss_ctx_with_source("@if $cond { color: red; }\n.foo {}");
        let d = ScssAtIfClosingBraceNewlineAfter.check_root(&[], &ctx);
        assert!(d.is_empty());
    }

    #[test]
    fn allows_else_on_same_line() {
        let ctx = scss_ctx_with_source("@if $cond { color: red; } @else { color: blue; }\n");
        let d = ScssAtIfClosingBraceNewlineAfter.check_root(&[], &ctx);
        assert!(d.is_empty());
    }

    #[test]
    fn skips_non_scss() {
        let ctx = RuleContext {
            file_path: "t.css",
            source: "@if { } .foo {}",
            syntax: Syntax::Css,
            options: None,
        };
        assert!(ScssAtIfClosingBraceNewlineAfter.check_root(&[], &ctx).is_empty());
    }
}
