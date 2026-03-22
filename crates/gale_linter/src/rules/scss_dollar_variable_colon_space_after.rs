use gale_css_parser::{CssNode, Syntax};
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require a single space or disallow whitespace after the colon in `$variable`
/// declarations.
///
/// Default: `"always"` (a single space after the colon).
///
/// ```scss
/// // Good (always)
/// $var: value;
///
/// // Bad (always)
/// $var:value;
/// $var:  value;
/// ```
///
/// Equivalent to `scss/dollar-variable-colon-space-after`.
pub struct ScssDollarVariableColonSpaceAfter;

impl Rule for ScssDollarVariableColonSpaceAfter {
    fn name(&self) -> &'static str {
        "scss/dollar-variable-colon-space-after"
    }

    fn description(&self) -> &'static str {
        "Require or disallow a space after the colon in $variable declarations"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, _nodes: &[CssNode], ctx: &RuleContext) -> Vec<Diagnostic> {
        if !matches!(ctx.syntax, Syntax::Scss | Syntax::Sass) {
            return vec![];
        }

        let option = ctx.primary_option_str().unwrap_or("always");
        let mut diagnostics = Vec::new();

        // Scan source text for $variable: patterns
        let source = ctx.source;
        let bytes = source.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            // Look for `$`
            if bytes[i] != b'$' {
                i += 1;
                continue;
            }

            // Collect variable name
            let dollar_pos = i;
            i += 1;
            while i < len
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-' || bytes[i] == b'_')
            {
                i += 1;
            }

            // Check for `:` after variable name (skip whitespace before colon)
            let mut j = i;
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }

            if j >= len || bytes[j] != b':' {
                i = j;
                continue;
            }

            // Found `$var:` — now check space after colon
            let colon_pos = j;
            let after_colon = colon_pos + 1;

            match option {
                "always" => {
                    if after_colon >= len || bytes[after_colon] != b' ' {
                        diagnostics.push(
                            Diagnostic::new(
                                self.name(),
                                "Expected single space after \":\" in $variable declaration"
                                    .to_string(),
                            )
                            .severity(self.default_severity())
                            .span(Span::new(dollar_pos, colon_pos - dollar_pos + 1)),
                        );
                    } else if after_colon + 1 < len && bytes[after_colon + 1] == b' ' {
                        // Multiple spaces
                        diagnostics.push(
                            Diagnostic::new(
                                self.name(),
                                "Expected single space after \":\" in $variable declaration"
                                    .to_string(),
                            )
                            .severity(self.default_severity())
                            .span(Span::new(dollar_pos, colon_pos - dollar_pos + 1)),
                        );
                    }
                }
                "never" => {
                    if after_colon < len
                        && (bytes[after_colon] == b' ' || bytes[after_colon] == b'\t')
                    {
                        diagnostics.push(
                            Diagnostic::new(
                                self.name(),
                                "Unexpected space after \":\" in $variable declaration".to_string(),
                            )
                            .severity(self.default_severity())
                            .span(Span::new(dollar_pos, colon_pos - dollar_pos + 1)),
                        );
                    }
                }
                "always-single-line" | "at-least-one-space" => {
                    // Simplified: just require at least one space
                    if after_colon >= len || bytes[after_colon] != b' ' {
                        diagnostics.push(
                            Diagnostic::new(
                                self.name(),
                                "Expected space after \":\" in $variable declaration".to_string(),
                            )
                            .severity(self.default_severity())
                            .span(Span::new(dollar_pos, colon_pos - dollar_pos + 1)),
                        );
                    }
                }
                _ => {}
            }

            i = after_colon + 1;
        }

        diagnostics
    }
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
    fn allows_single_space() {
        let ctx = scss_ctx_with_source("$color: red;");
        let d = ScssDollarVariableColonSpaceAfter.check_root(&[], &ctx);
        assert!(d.is_empty());
    }

    #[test]
    fn reports_no_space() {
        let ctx = scss_ctx_with_source("$color:red;");
        let d = ScssDollarVariableColonSpaceAfter.check_root(&[], &ctx);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn reports_multiple_spaces() {
        let ctx = scss_ctx_with_source("$color:  red;");
        let d = ScssDollarVariableColonSpaceAfter.check_root(&[], &ctx);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn skips_non_scss() {
        let ctx = RuleContext {
            file_path: "t.css",
            source: "$color:red;",
            syntax: Syntax::Css,
            options: None,
        };
        assert!(
            ScssDollarVariableColonSpaceAfter
                .check_root(&[], &ctx)
                .is_empty()
        );
    }
}
