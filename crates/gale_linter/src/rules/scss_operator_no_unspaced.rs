use gale_diagnostics::Severity;

use crate::rule::Rule;

/// Disallow unspaced operators.
///
/// Stub — always passes. Full implementation requires source-text scanning.
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
}
