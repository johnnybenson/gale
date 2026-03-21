use gale_css_parser::{CssNode, Syntax, parse};
use gale_diagnostics::LintResult;

use crate::registry::RuleRegistry;
use crate::rule::RuleContext;

/// The main lint runner that applies enabled rules to parsed CSS.
pub struct LintRunner {
    registry: RuleRegistry,
    enabled_rules: Vec<String>,
}

impl LintRunner {
    /// Create a new runner with the given registry and list of enabled rule names.
    pub fn new(registry: RuleRegistry, enabled_rules: Vec<String>) -> Self {
        Self {
            registry,
            enabled_rules,
        }
    }

    /// Parse and lint a CSS source string, returning all diagnostics.
    pub fn lint_source(&self, source: &str, file_path: &str, syntax: Syntax) -> LintResult {
        let parse_result = match parse(source, syntax) {
            Ok(result) => result,
            Err(_err) => {
                return LintResult::new(file_path, source, vec![]);
            }
        };

        let context = RuleContext {
            file_path,
            source,
            syntax,
        };

        let mut diagnostics = Vec::new();

        // Collect enabled rules from the registry.
        let active_rules: Vec<&dyn crate::rule::Rule> = self
            .enabled_rules
            .iter()
            .filter_map(|name| self.registry.get(name))
            .collect();

        // Run document-level checks (check_root).
        for rule in &active_rules {
            let mut results = rule.check_root(&parse_result.nodes, &context);
            diagnostics.append(&mut results);
        }

        // Walk each top-level node for per-node checks.
        for node in &parse_result.nodes {
            walk_node(node, &active_rules, &context, &mut diagnostics);
        }

        // Set file_path on all diagnostics.
        for diag in &mut diagnostics {
            if diag.file_path.is_empty() {
                diag.file_path = file_path.to_string();
            }
        }

        // Sort diagnostics by position for consistent output.
        diagnostics.sort_by_key(|d| d.span.offset);

        LintResult::new(file_path, source, diagnostics)
    }
}

/// Recursively walk the AST, invoking each rule's `check` on every node.
fn walk_node(
    node: &CssNode,
    rules: &[&dyn crate::rule::Rule],
    context: &RuleContext,
    diagnostics: &mut Vec<gale_diagnostics::Diagnostic>,
) {
    // Run rules on this node.
    for rule in rules {
        let mut results = rule.check(node, context);
        diagnostics.append(&mut results);
    }

    // Recurse into children based on node type.
    match node {
        CssNode::Style(style_rule) => {
            for child in &style_rule.children {
                let child_node = CssNode::Style(child.clone());
                walk_node(&child_node, rules, context, diagnostics);
            }
        }
        CssNode::AtRule(at_rule) => {
            for child in &at_rule.children {
                walk_node(child, rules, context, diagnostics);
            }
        }
        CssNode::Comment(_) | CssNode::Declaration(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::RuleRegistry;

    #[test]
    fn lint_empty_block() {
        let registry = RuleRegistry::default();
        let runner = LintRunner::new(registry, vec!["block-no-empty".to_string()]);
        let result = runner.lint_source("a { }", "test.css", Syntax::Css);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].rule_name, "block-no-empty");
        assert_eq!(result.diagnostics[0].message, "Unexpected empty block");
    }

    #[test]
    fn lint_non_empty_block() {
        let registry = RuleRegistry::default();
        let runner = LintRunner::new(registry, vec!["block-no-empty".to_string()]);
        let result = runner.lint_source("a { color: red; }", "test.css", Syntax::Css);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn disabled_rule_not_run() {
        let registry = RuleRegistry::default();
        let runner = LintRunner::new(registry, vec![]);
        let result = runner.lint_source("a { }", "test.css", Syntax::Css);
        assert!(result.diagnostics.is_empty());
    }
}
