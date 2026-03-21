use std::time::Instant;

use gale_css_parser::{CssNode, Syntax, parse};
use gale_diagnostics::LintResult;

use crate::registry::RuleRegistry;
use crate::rule::RuleContext;

/// Returns `true` when the `GALE_DEBUG_PERF` environment variable is set to `"1"`.
fn perf_enabled() -> bool {
    std::env::var("GALE_DEBUG_PERF")
        .map(|v| v == "1")
        .unwrap_or(false)
}

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
        let debug = perf_enabled();

        let t0 = Instant::now();
        let parse_result = match parse(source, syntax) {
            Ok(result) => result,
            Err(_err) => {
                return LintResult::new(file_path, source, vec![]);
            }
        };
        if debug {
            eprintln!("[perf] parse: {:.3}s", t0.elapsed().as_secs_f64());
        }

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
        let t1 = Instant::now();
        for rule in &active_rules {
            let tr = Instant::now();
            let mut results = rule.check_root(&parse_result.nodes, &context);
            if debug {
                let elapsed = tr.elapsed().as_secs_f64();
                if elapsed > 0.001 {
                    eprintln!("[perf] check_root {}: {:.3}s", rule.name(), elapsed);
                }
            }
            diagnostics.append(&mut results);
        }
        if debug {
            eprintln!("[perf] check_root total: {:.3}s", t1.elapsed().as_secs_f64());
        }

        // Walk each top-level node for per-node checks.
        let t2 = Instant::now();
        for node in &parse_result.nodes {
            walk_node(node, &active_rules, &context, &mut diagnostics);
        }
        if debug {
            eprintln!("[perf] walk: {:.3}s", t2.elapsed().as_secs_f64());
        }

        // Set file_path on all diagnostics.
        let t3 = Instant::now();
        for diag in &mut diagnostics {
            if diag.file_path.is_empty() {
                diag.file_path = file_path.to_string();
            }
        }
        if debug {
            eprintln!("[perf] set_file_path: {:.3}s", t3.elapsed().as_secs_f64());
        }

        // Sort diagnostics by position for consistent output.
        let t4 = Instant::now();
        diagnostics.sort_by_key(|d| d.span.offset);
        if debug {
            eprintln!("[perf] sort: {:.3}s", t4.elapsed().as_secs_f64());
            eprintln!("[perf] total diagnostics: {}", diagnostics.len());
        }

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
