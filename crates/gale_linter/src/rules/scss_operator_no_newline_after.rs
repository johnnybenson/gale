use gale_diagnostics::Severity;

use crate::rule::Rule;

/// Disallow newlines after operators (+, -, *, /).
///
/// Stub — always passes. Full implementation requires source-text scanning.
pub struct ScssOperatorNoNewlineAfter;

impl Rule for ScssOperatorNoNewlineAfter {
    fn name(&self) -> &'static str {
        "scss/operator-no-newline-after"
    }

    fn description(&self) -> &'static str {
        "Disallow newlines after Sass operators"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }
}
