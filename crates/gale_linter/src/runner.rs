use std::time::Instant;

use gale_css_parser::{CssNode, Syntax, parse};
use gale_diagnostics::{LintResult, SourceLineIndex};

use crate::registry::RuleRegistry;
use crate::rule::RuleContext;

// ---------------------------------------------------------------------------
// Inline disable-comment support
// ---------------------------------------------------------------------------

/// A range in the source where certain (or all) rules are disabled.
#[derive(Debug)]
struct DisabledRange {
    /// Byte offset where the disable starts.
    start: usize,
    /// Byte offset where the disable ends (exclusive). `usize::MAX` means EOF.
    end: usize,
    /// `None` → all rules disabled; `Some(name)` → only that rule.
    rule: Option<String>,
}

/// Scan `source` for gale / stylelint disable comments and return disabled ranges.
fn collect_disabled_ranges(source: &str, line_index: &SourceLineIndex) -> Vec<DisabledRange> {
    let mut ranges: Vec<DisabledRange> = Vec::new();

    // Track open "disable" directives: (byte_offset, Option<rule_name>)
    let mut open_disables: Vec<(usize, Option<String>)> = Vec::new();

    // We scan for `/* ... */` comments manually so we don't rely on the parser
    // (comments inside values, etc. would be stripped by the parser).
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i + 1 < len {
        if bytes[i] == b'/' && bytes[i + 1] == b'*' {
            // Found comment start — find the end.
            let comment_start = i;
            if let Some(end_pos) = find_comment_end(bytes, i + 2) {
                let comment_end = end_pos + 2; // past `*/`
                let inner = &source[comment_start + 2..end_pos];
                let trimmed = inner.trim();

                process_directive(
                    trimmed,
                    comment_start,
                    comment_end,
                    source,
                    line_index,
                    &mut open_disables,
                    &mut ranges,
                );

                i = comment_end;
            } else {
                break; // unterminated comment
            }
        } else {
            i += 1;
        }
    }

    // Close any still-open disables at EOF.
    for (start, rule) in open_disables {
        ranges.push(DisabledRange {
            start,
            end: len,
            rule,
        });
    }

    ranges
}

fn find_comment_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut j = from;
    while j + 1 < bytes.len() {
        if bytes[j] == b'*' && bytes[j + 1] == b'/' {
            return Some(j);
        }
        j += 1;
    }
    None
}

fn process_directive(
    trimmed: &str,
    comment_start: usize,
    comment_end: usize,
    source: &str,
    line_index: &SourceLineIndex,
    open_disables: &mut Vec<(usize, Option<String>)>,
    ranges: &mut Vec<DisabledRange>,
) {
    // Try both prefixes: gale-* and stylelint-*
    for prefix in &["gale-", "stylelint-"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            handle_directive(
                rest,
                comment_start,
                comment_end,
                source,
                line_index,
                open_disables,
                ranges,
            );
            return;
        }
    }
}

fn handle_directive(
    rest: &str,
    comment_start: usize,
    comment_end: usize,
    source: &str,
    line_index: &SourceLineIndex,
    open_disables: &mut Vec<(usize, Option<String>)>,
    ranges: &mut Vec<DisabledRange>,
) {
    if let Some(rule_part) = rest.strip_prefix("disable-next-line") {
        // disable-next-line [rule-name]
        let rule_name = parse_rule_name(rule_part);
        let (comment_line, _) = line_index.offset_to_location(comment_start);
        let next_line = comment_line + 1;
        let (next_start, next_end) = line_byte_range(source, next_line);
        ranges.push(DisabledRange {
            start: next_start,
            end: next_end,
            rule: rule_name,
        });
    } else if let Some(rule_part) = rest.strip_prefix("enable") {
        // enable [rule-name]
        let rule_name = parse_rule_name(rule_part);
        close_disable(open_disables, ranges, comment_start, &rule_name);
    } else if let Some(rule_part) = rest.strip_prefix("disable") {
        // disable [rule-name]
        let rule_name = parse_rule_name(rule_part);
        open_disables.push((comment_end, rule_name));
    }
}

/// Parse an optional rule name from the text after a directive keyword.
/// E.g. " rule-name " → Some("rule-name"), "" → None.
fn parse_rule_name(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Close the most-recent matching open disable.
fn close_disable(
    open_disables: &mut Vec<(usize, Option<String>)>,
    ranges: &mut Vec<DisabledRange>,
    end_offset: usize,
    rule_name: &Option<String>,
) {
    // Find the last matching open disable (same rule or both None).
    if let Some(idx) = open_disables
        .iter()
        .rposition(|(_, r)| r == rule_name)
    {
        let (start, rule) = open_disables.remove(idx);
        ranges.push(DisabledRange {
            start,
            end: end_offset,
            rule,
        });
    }
}

/// Return (start_byte, end_byte) for 1-indexed `line_number`.
fn line_byte_range(source: &str, line_number: usize) -> (usize, usize) {
    let mut current_line = 1usize;
    let mut line_start = 0usize;

    for (i, b) in source.bytes().enumerate() {
        if current_line == line_number {
            // Find end of this line.
            let mut end = i;
            for (j, b2) in source.bytes().enumerate().skip(i) {
                if b2 == b'\n' {
                    end = j + 1; // include the newline
                    return (line_start, end);
                }
                end = j + 1;
            }
            return (line_start, end);
        }
        if b == b'\n' {
            current_line += 1;
            line_start = i + 1;
        }
    }
    // If the requested line is beyond EOF, return an empty range at end.
    (source.len(), source.len())
}

/// Returns `true` if the diagnostic's span falls within any disabled range.
fn is_disabled(diag: &gale_diagnostics::Diagnostic, ranges: &[DisabledRange]) -> bool {
    let offset = diag.span.offset;
    for r in ranges {
        if offset >= r.start && offset < r.end {
            match &r.rule {
                None => return true, // all rules disabled
                Some(name) if name == &diag.rule_name => return true,
                _ => {}
            }
        }
    }
    false
}

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

        // Filter diagnostics based on inline disable comments.
        let t4 = Instant::now();
        let line_index = SourceLineIndex::build(source);
        let disabled_ranges = collect_disabled_ranges(source, &line_index);
        if !disabled_ranges.is_empty() {
            diagnostics.retain(|d| !is_disabled(d, &disabled_ranges));
        }
        if debug {
            eprintln!("[perf] disable-filter: {:.3}s", t4.elapsed().as_secs_f64());
        }

        // Sort diagnostics by position for consistent output.
        let t5 = Instant::now();
        diagnostics.sort_by_key(|d| d.span.offset);
        if debug {
            eprintln!("[perf] sort: {:.3}s", t5.elapsed().as_secs_f64());
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

    // -- Inline disable comment tests --

    #[test]
    fn gale_disable_all() {
        let registry = RuleRegistry::default();
        let runner = LintRunner::new(registry, vec!["block-no-empty".to_string()]);
        let src = "/* gale-disable */\na { }";
        let result = runner.lint_source(src, "test.css", Syntax::Css);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn gale_disable_specific_rule() {
        let registry = RuleRegistry::default();
        let runner = LintRunner::new(registry, vec!["block-no-empty".to_string()]);
        let src = "/* gale-disable block-no-empty */\na { }";
        let result = runner.lint_source(src, "test.css", Syntax::Css);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn gale_disable_wrong_rule_still_reports() {
        let registry = RuleRegistry::default();
        let runner = LintRunner::new(registry, vec!["block-no-empty".to_string()]);
        let src = "/* gale-disable some-other-rule */\na { }";
        let result = runner.lint_source(src, "test.css", Syntax::Css);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn gale_disable_enable_block() {
        let registry = RuleRegistry::default();
        let runner = LintRunner::new(
            registry,
            vec!["block-no-empty".to_string()],
        );
        let src = "/* gale-disable */\na { }\n/* gale-enable */\nb { }";
        let result = runner.lint_source(src, "test.css", Syntax::Css);
        // First `a { }` is disabled, second `b { }` should be reported.
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn gale_disable_next_line() {
        let registry = RuleRegistry::default();
        let runner = LintRunner::new(registry, vec!["block-no-empty".to_string()]);
        let src = "/* gale-disable-next-line */\na { }\nb { }";
        let result = runner.lint_source(src, "test.css", Syntax::Css);
        // Only `a { }` is disabled; `b { }` still reported.
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn gale_disable_next_line_specific_rule() {
        let registry = RuleRegistry::default();
        let runner = LintRunner::new(registry, vec!["block-no-empty".to_string()]);
        let src = "/* gale-disable-next-line block-no-empty */\na { }";
        let result = runner.lint_source(src, "test.css", Syntax::Css);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn stylelint_disable_compat() {
        let registry = RuleRegistry::default();
        let runner = LintRunner::new(registry, vec!["block-no-empty".to_string()]);
        let src = "/* stylelint-disable */\na { }";
        let result = runner.lint_source(src, "test.css", Syntax::Css);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn stylelint_disable_next_line_compat() {
        let registry = RuleRegistry::default();
        let runner = LintRunner::new(registry, vec!["block-no-empty".to_string()]);
        let src = "/* stylelint-disable-next-line */\na { }\nb { }";
        let result = runner.lint_source(src, "test.css", Syntax::Css);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
