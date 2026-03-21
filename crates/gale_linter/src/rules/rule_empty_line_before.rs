use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require or disallow an empty line before rules.
///
/// Equivalent to Stylelint's `rule-empty-line-before` rule.
/// Supports primary options: "always", "never", "always-multi-line", "never-multi-line".
/// Supports secondary options: `except` and `ignore`.
pub struct RuleEmptyLineBefore;

impl Rule for RuleEmptyLineBefore {
    fn name(&self) -> &'static str {
        "rule-empty-line-before"
    }

    fn description(&self) -> &'static str {
        "Require or disallow an empty line before rules"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, nodes: &[CssNode], ctx: &RuleContext) -> Vec<Diagnostic> {
        let opts = Options::from_ctx(ctx);
        let mut diags = Vec::new();
        check_nodes(self, nodes, ctx, &opts, true, &mut diags);
        diags
    }
}

/// Parsed options for rule-empty-line-before.
struct Options {
    primary: PrimaryOption,
    except_first_nested: bool,
    except_after_single_line_comment: bool,
    except_after_rule: bool,
    ignore_after_comment: bool,
    ignore_first_nested: bool,
    ignore_inside_block: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PrimaryOption {
    Always,
    Never,
    AlwaysMultiLine,
    NeverMultiLine,
}

impl Options {
    fn from_ctx(ctx: &RuleContext) -> Self {
        let mut opts = Options {
            primary: PrimaryOption::Always,
            except_first_nested: false,
            except_after_single_line_comment: false,
            except_after_rule: false,
            ignore_after_comment: false,
            ignore_first_nested: false,
            ignore_inside_block: false,
        };

        let Some(value) = ctx.options else {
            return opts;
        };

        // Options can be:
        // - A string: "always" / "never" / "always-multi-line" / "never-multi-line"
        // - An array: ["always", { except: [...], ignore: [...] }]
        match value {
            serde_json::Value::String(s) => {
                opts.primary = parse_primary(s);
            }
            serde_json::Value::Array(arr) => {
                if let Some(primary_str) = arr.first().and_then(|v| v.as_str()) {
                    opts.primary = parse_primary(primary_str);
                }
                if let Some(secondary) = arr.get(1) {
                    parse_secondary(&mut opts, secondary);
                }
            }
            _ => {}
        }

        opts
    }
}

fn parse_primary(s: &str) -> PrimaryOption {
    match s {
        "never" => PrimaryOption::Never,
        "always-multi-line" => PrimaryOption::AlwaysMultiLine,
        "never-multi-line" => PrimaryOption::NeverMultiLine,
        _ => PrimaryOption::Always,
    }
}

fn parse_secondary(opts: &mut Options, value: &serde_json::Value) {
    if let Some(except) = value.get("except").and_then(|v| v.as_array()) {
        for item in except {
            if let Some(s) = item.as_str() {
                match s {
                    "first-nested" => opts.except_first_nested = true,
                    "after-single-line-comment" => opts.except_after_single_line_comment = true,
                    "after-rule" => opts.except_after_rule = true,
                    _ => {}
                }
            }
        }
    }
    if let Some(ignore) = value.get("ignore").and_then(|v| v.as_array()) {
        for item in ignore {
            if let Some(s) = item.as_str() {
                match s {
                    "after-comment" => opts.ignore_after_comment = true,
                    "first-nested" => opts.ignore_first_nested = true,
                    "inside-block" => opts.ignore_inside_block = true,
                    _ => {}
                }
            }
        }
    }
}

/// Check if the text before a node has an empty line (double newline).
///
/// An "empty line" is a line containing only whitespace.  For example,
/// `"  ;\n  \n  &"` has an empty line before `&` even though the blank
/// line contains spaces.
fn has_empty_line_before(source: &str, offset: usize) -> bool {
    if offset == 0 || offset > source.len() {
        return false;
    }
    let before = &source[..offset];

    // Find the start of the current line (skip back past the indentation
    // leading up to the node).  We want to look at complete lines only.
    let last_newline = before.rfind('\n');
    let Some(nl_pos) = last_newline else {
        return false; // No newline before → first line → no empty line.
    };

    // Now look at lines before `nl_pos` for a blank line.
    let before_lines = &before[..nl_pos];
    let mut found_blank = false;
    for line in before_lines.rsplit('\n') {
        let stripped = line.trim_matches(|c: char| c == ' ' || c == '\t' || c == '\r');
        if stripped.is_empty() {
            found_blank = true;
        } else {
            return found_blank;
        }
    }
    // If we exhausted all lines and they were all blank, there's no prior
    // content to separate from → no empty line in the meaningful sense.
    false
}

/// Check if a node is the first meaningful (non-comment) child in a block.
fn is_first_nested_in_list(nodes: &[CssNode], index: usize) -> bool {
    if index == 0 {
        return true;
    }
    nodes[..index]
        .iter()
        .all(|n| matches!(n, CssNode::Comment(_)))
}

/// Check if a Style node at `index` is the first `CssNode::Style` in the list.
/// Declarations, at-rules, and comments that appear before it are ignored.
fn is_first_style_in_list(nodes: &[CssNode], index: usize) -> bool {
    nodes[..index]
        .iter()
        .all(|n| !matches!(n, CssNode::Style(_)))
}

/// Check if a node is first-nested by looking at the source (follows an opening brace).
///
/// Skips over comments (both `//` line comments and `/* */` block comments)
/// that may appear between the opening `{` and the first rule, which is common
/// in SCSS.
fn is_first_nested_by_source(source: &str, offset: usize) -> bool {
    if offset == 0 || offset > source.len() {
        return false;
    }
    let before = &source[..offset];
    let trimmed = before.trim();
    if trimmed.ends_with('{') {
        return true;
    }
    // Walk backwards through the source, skipping comments and whitespace,
    // to see if we eventually reach an opening brace.
    let bytes = before.as_bytes();
    let mut pos = before.len();
    loop {
        // Skip trailing whitespace
        while pos > 0 && matches!(bytes[pos - 1], b' ' | b'\t' | b'\n' | b'\r') {
            pos -= 1;
        }
        if pos == 0 {
            return false;
        }
        // Check for end of a line comment: walk back to find `//` at start of this line
        // First, find the start of the current line
        let line_end = pos;
        let line_start = before[..pos].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let line = before[line_start..line_end].trim();
        if line.starts_with("//") {
            // This is a line comment; skip past it
            pos = line_start;
            continue;
        }
        // If the line contains a `//` comment after a `{`, strip the comment
        // and check the remaining content.
        if let Some(comment_pos) = line.find("//") {
            let before_comment = line[..comment_pos].trim();
            if before_comment.ends_with('{') {
                return true;
            }
        }
        // Check for end of a block comment `*/`
        if pos >= 2 && &before[pos - 2..pos] == "*/" {
            // Find the matching `/*`
            if let Some(open) = before[..pos - 2].rfind("/*") {
                pos = open;
                continue;
            }
            return false;
        }
        // Not a comment — check if it's `{`
        return bytes[pos - 1] == b'{';
    }
}

/// Check if the previous sibling is a single-line comment.
fn prev_is_single_line_comment(nodes: &[CssNode], index: usize) -> bool {
    if index == 0 {
        return false;
    }
    match &nodes[index - 1] {
        CssNode::Comment(c) => c.is_line,
        _ => false,
    }
}

/// Check if the previous sibling is any comment (AST-based).
fn prev_is_comment(nodes: &[CssNode], index: usize) -> bool {
    if index == 0 {
        return false;
    }
    matches!(&nodes[index - 1], CssNode::Comment(_))
}

/// Source-based check: is the non-empty line immediately before `offset` a comment?
/// This catches SCSS `//` comments that may not be in the AST.
fn prev_line_is_comment(source: &str, offset: usize) -> bool {
    if offset == 0 || offset > source.len() {
        return false;
    }
    let before = &source[..offset];
    // Walk backwards past the newline that ends the preceding line, then find the line content.
    let trimmed = before.trim_end_matches([' ', '\t']);
    let trimmed = trimmed.strip_suffix('\n').unwrap_or(trimmed);
    let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
    // Now find the start of this line.
    let line_start = trimmed.rfind('\n').map(|p| p + 1).unwrap_or(0);
    let line = trimmed[line_start..].trim();
    line.starts_with("//") || (line.starts_with("/*") && line.ends_with("*/"))
}

/// Source-based check: is the non-empty line immediately before `offset` a
/// single-line (`//`) comment?  This catches SCSS `//` comments that may not
/// appear in the AST.  Unlike `prev_line_is_comment` this intentionally does
/// **not** match `/* … */` block comments.
fn prev_line_is_single_line_comment(source: &str, offset: usize) -> bool {
    if offset == 0 || offset > source.len() {
        return false;
    }
    let before = &source[..offset];
    let trimmed = before.trim_end_matches([' ', '\t']);
    let trimmed = trimmed.strip_suffix('\n').unwrap_or(trimmed);
    let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
    let line_start = trimmed.rfind('\n').map(|p| p + 1).unwrap_or(0);
    let line = trimmed[line_start..].trim();
    line.starts_with("//")
}

/// Check whether a selector contains preprocessor interpolation (`#{`, `@{`)
/// or other non-standard-syntax patterns that Stylelint would skip.
fn has_interpolation(selector: &str) -> bool {
    selector.contains("#{") || selector.contains("@{")
}

/// Check if the previous sibling is a style rule.
fn prev_is_rule(nodes: &[CssNode], index: usize) -> bool {
    if index == 0 {
        return false;
    }
    matches!(&nodes[index - 1], CssNode::Style(_))
}

/// Check if a style rule is multi-line by looking at the source.
fn is_rule_multi_line(source: &str, span: &gale_css_parser::Span) -> bool {
    let start = span.offset;
    let end = (start + span.length).min(source.len());
    if start >= source.len() {
        return false;
    }
    source[start..end].contains('\n')
}

fn check_nodes(
    rule_impl: &RuleEmptyLineBefore,
    nodes: &[CssNode],
    ctx: &RuleContext,
    opts: &Options,
    is_root: bool,
    diags: &mut Vec<Diagnostic>,
) {
    for (i, node) in nodes.iter().enumerate() {
        if let CssNode::Style(style) = node {
            // Stylelint skips rules whose selectors contain SCSS/Less
            // interpolation (e.g. `.#{$prefix}--foo`) because they are not
            // "standard syntax".  Match that behavior.
            if has_interpolation(&style.selector) {
                check_children(rule_impl, style, ctx, opts, diags);
                continue;
            }

            let offset = style.span.offset;

            // Determine if this is first-nested: the first meaningful content after
            // the opening `{` of the parent block. We rely on source analysis
            // rather than the node list, because the node list may not include
            // all sibling types (declarations, at-rules) that appear before this
            // style rule in the source.
            //
            // For the document root, we use the list-based check since there is no
            // opening `{` — the first rule (or first after only comments) should be
            // skipped (nothing before it to separate from).
            let first_nested = if is_root {
                is_first_nested_in_list(nodes, i)
            } else {
                is_first_nested_by_source(ctx.source, offset)
            };

            // At the document root, the very first rule (or first after only comments)
            // is never checked — there's nothing before it to separate from.
            // This matches Stylelint behavior.
            if is_root && first_nested {
                check_children(rule_impl, style, ctx, opts, diags);
                continue;
            }

            // ignore: ["first-nested"] — skip entirely
            if opts.ignore_first_nested && first_nested {
                check_children(rule_impl, style, ctx, opts, diags);
                continue;
            }

            // ignore: ["after-comment"] — skip if preceded by a comment.
            // Use both AST-based check and source-based check (for SCSS //
            // comments that may not be represented in the AST).
            if opts.ignore_after_comment
                && (prev_is_comment(nodes, i) || prev_line_is_comment(ctx.source, offset))
            {
                check_children(rule_impl, style, ctx, opts, diags);
                continue;
            }

            let has_empty = has_empty_line_before(ctx.source, offset);

            // Determine the expectation based on primary option.
            // For *-multi-line variants, skip single-line rules entirely.
            let is_multi = is_rule_multi_line(ctx.source, &style.span);

            let expects_empty = match opts.primary {
                PrimaryOption::Always => true,
                PrimaryOption::Never => false,
                PrimaryOption::AlwaysMultiLine => {
                    if !is_multi {
                        check_children(rule_impl, style, ctx, opts, diags);
                        continue;
                    }
                    true
                }
                PrimaryOption::NeverMultiLine => {
                    if !is_multi {
                        check_children(rule_impl, style, ctx, opts, diags);
                        continue;
                    }
                    false
                }
            };

            // Apply exceptions — flip the expectation at most once.
            // Stylelint evaluates exceptions in order and stops after the
            // first match (PR #2920).  This prevents double-flipping when
            // multiple exceptions apply simultaneously (e.g. a rule that is
            // both first-nested AND after a single-line comment).
            let mut expectation = expects_empty;

            let exception_matched = if opts.except_first_nested && first_nested {
                true
            } else if opts.except_after_single_line_comment
                && (prev_is_single_line_comment(nodes, i)
                    || prev_line_is_single_line_comment(ctx.source, offset))
            {
                true
            } else {
                opts.except_after_rule && prev_is_rule(nodes, i)
            };

            if exception_matched {
                expectation = !expectation;
            }

            // Check and report
            if expectation && !has_empty {
                diags.push(
                    Diagnostic::new(
                        rule_impl.name(),
                        format!("Expected empty line before rule \"{}\"", style.selector),
                    )
                    .severity(rule_impl.default_severity())
                    .span(Span::new(style.span.offset, style.span.length)),
                );
            } else if !expectation && has_empty {
                diags.push(
                    Diagnostic::new(
                        rule_impl.name(),
                        format!("Unexpected empty line before rule \"{}\"", style.selector),
                    )
                    .severity(rule_impl.default_severity())
                    .span(Span::new(style.span.offset, style.span.length)),
                );
            }

            // Recurse into nested rules within this style rule
            check_children(rule_impl, style, ctx, opts, diags);
        }

        // Recurse into at-rules (but skip @keyframes — Stylelint does not
        // check rule-empty-line-before inside keyframe blocks).
        if let CssNode::AtRule(at_rule) = node {
            let at_name = at_rule.name.to_lowercase();
            if at_name != "keyframes" && !at_name.ends_with("-keyframes") {
                check_nodes(rule_impl, &at_rule.children, ctx, opts, false, diags);
            }
        }
    }
}

fn check_children(
    rule_impl: &RuleEmptyLineBefore,
    style: &gale_css_parser::StyleRule,
    ctx: &RuleContext,
    opts: &Options,
    diags: &mut Vec<Diagnostic>,
) {
    if !style.children.is_empty() {
        let child_nodes: Vec<CssNode> = style
            .children
            .iter()
            .map(|sr| CssNode::Style(sr.clone()))
            .collect();
        check_nodes(rule_impl, &child_nodes, ctx, opts, false, diags);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn make_ctx(source: &str) -> RuleContext {
        RuleContext {
            file_path: "t.css",
            source,
            syntax: Syntax::Css,
            options: None,
        }
    }

    fn make_ctx_with_options<'a>(source: &'a str, options: &'a serde_json::Value) -> RuleContext<'a> {
        RuleContext {
            file_path: "t.css",
            source,
            syntax: Syntax::Css,
            options: Some(options),
        }
    }

    #[test]
    fn reports_missing_empty_line_before_rule() {
        // Default: "always" with no options
        let src = "a { color: red; }\nb { color: blue; }";
        let b_offset = src.find("b {").unwrap();
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(4, 10),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(0, 17),
            }),
            CssNode::Style(StyleRule {
                selector: "b".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "blue".to_string(),
                    span: ParserSpan::new(b_offset + 4, 11),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(b_offset, 18),
            }),
        ];
        let d = RuleEmptyLineBefore.check_root(&nodes, &make_ctx(src));
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("b"));
    }

    #[test]
    fn allows_empty_line_before_rule() {
        let src = "a { color: red; }\n\nb { color: blue; }";
        let b_offset = src.find("b {").unwrap();
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(4, 10),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(0, 17),
            }),
            CssNode::Style(StyleRule {
                selector: "b".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "blue".to_string(),
                    span: ParserSpan::new(b_offset + 4, 11),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(b_offset, 18),
            }),
        ];
        let d = RuleEmptyLineBefore.check_root(&nodes, &make_ctx(src));
        assert!(d.is_empty());
    }

    #[test]
    fn except_first_nested_allows_no_empty_line() {
        // With except: ["first-nested"], the first rule inside a block
        // should NOT require an empty line (flipped from "always")
        let opts = serde_json::json!(["always", {"except": ["first-nested"]}]);
        let src = "a {\n  b { color: red; }\n}";
        let b_offset = src.find("b {").unwrap();
        let nodes = vec![CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![],
            children: vec![StyleRule {
                selector: "b".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(b_offset + 4, 10),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(b_offset, 18),
            }],
            span: ParserSpan::new(0, src.len()),
        })];
        let d = RuleEmptyLineBefore.check_root(&nodes, &make_ctx_with_options(src, &opts));
        assert!(d.is_empty(), "first-nested exception should suppress diagnostic");
    }

    #[test]
    fn except_after_single_line_comment_flips() {
        // With except: ["after-single-line-comment"], a rule after a // comment
        // should NOT require an empty line (flipped from "always")
        let opts = serde_json::json!(["always", {"except": ["after-single-line-comment"]}]);
        let src = "a { color: red; }\n// comment\nb { color: blue; }";
        let b_offset = src.find("b {").unwrap();
        let comment_offset = src.find("//").unwrap();
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 17),
            }),
            CssNode::Comment(gale_css_parser::Comment {
                is_line: true,
                text: " comment".to_string(),
                span: ParserSpan::new(comment_offset, 10),
            }),
            CssNode::Style(StyleRule {
                selector: "b".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(b_offset, 18),
            }),
        ];
        let ctx = RuleContext {
            file_path: "t.scss",
            source: src,
            syntax: Syntax::Scss,
            options: Some(&opts),
        };
        let d = RuleEmptyLineBefore.check_root(&nodes, &ctx);
        assert!(d.is_empty(), "after-single-line-comment exception should suppress diagnostic");
    }

    #[test]
    fn ignore_after_comment_skips() {
        // With ignore: ["after-comment"], a rule after a comment should be ignored entirely
        let opts = serde_json::json!(["always", {"ignore": ["after-comment"]}]);
        let src = "/* comment */\nb { color: blue; }";
        let b_offset = src.find("b {").unwrap();
        let nodes = vec![
            CssNode::Comment(gale_css_parser::Comment {
                is_line: false,
                text: " comment ".to_string(),
                span: ParserSpan::new(0, 13),
            }),
            CssNode::Style(StyleRule {
                selector: "b".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(b_offset, 18),
            }),
        ];
        let d = RuleEmptyLineBefore.check_root(&nodes, &make_ctx_with_options(src, &opts));
        assert!(d.is_empty(), "ignore after-comment should skip the check");
    }

    #[test]
    fn never_mode_reports_empty_line() {
        let opts = serde_json::json!(["never"]);
        let src = "a { color: red; }\n\nb { color: blue; }";
        let b_offset = src.find("b {").unwrap();
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 17),
            }),
            CssNode::Style(StyleRule {
                selector: "b".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(b_offset, 18),
            }),
        ];
        let d = RuleEmptyLineBefore.check_root(&nodes, &make_ctx_with_options(src, &opts));
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("Unexpected"));
    }
}
