use gale_css_parser::{CssNode, Syntax};
use gale_diagnostics::{Diagnostic, Severity};

/// Context passed to each rule when checking a node.
pub struct RuleContext<'a> {
    /// The path to the file being linted.
    pub file_path: &'a str,
    /// The full source text of the file.
    pub source: &'a str,
    /// The CSS syntax variant.
    pub syntax: Syntax,
}

/// A single lint rule that can inspect CSS AST nodes and emit diagnostics.
pub trait Rule: Send + Sync {
    /// Unique rule name, e.g. "block-no-empty".
    fn name(&self) -> &'static str;

    /// Human-readable description of what this rule checks.
    fn description(&self) -> &'static str;

    /// The default severity for diagnostics produced by this rule.
    fn default_severity(&self) -> Severity;

    /// Check a single CSS node and return any diagnostics.
    fn check(&self, node: &CssNode, context: &RuleContext) -> Vec<Diagnostic> {
        let _ = (node, context);
        vec![]
    }

    /// Check the entire document (all top-level nodes). Used for rules that need
    /// document-level context like duplicate detection or source-level checks.
    fn check_root(&self, _nodes: &[CssNode], _context: &RuleContext) -> Vec<Diagnostic> {
        vec![]
    }
}
