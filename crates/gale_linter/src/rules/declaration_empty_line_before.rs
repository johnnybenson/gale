use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require or disallow an empty line before declarations.
///
/// Equivalent to Stylelint's `declaration-empty-line-before` rule.
/// Supports primary options: "always", "never".
/// Supports secondary options: `except` and `ignore`.
pub struct DeclarationEmptyLineBefore;

impl Rule for DeclarationEmptyLineBefore {
    fn name(&self) -> &'static str {
        "declaration-empty-line-before"
    }

    fn description(&self) -> &'static str {
        "Require or disallow an empty line before declarations"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let opts = Options::from_ctx(ctx);
        let mut diags = Vec::new();

        for (_i, decl) in rule.declarations.iter().enumerate() {
            // Stylelint's declaration-empty-line-before only checks standard
            // property declarations.  Custom properties (starting with `--`)
            // are handled by `custom-property-empty-line-before` instead.
            if decl.property.starts_with("--") {
                continue;
            }

            // Skip SCSS variable declarations ($var: value)
            if decl.property.starts_with('$') {
                continue;
            }

            let decl_start = decl.span.offset;
            if decl_start == 0 || decl_start > ctx.source.len() {
                continue;
            }

            // Validate that the span actually points to this declaration in
            // the source.  The parser may produce incorrect offsets when the
            // property name also appears in the selector text.  We verify
            // that after the property name there is a colon (possibly with
            // whitespace), which distinguishes a real declaration from a
            // coincidental match in a selector.
            {
                let after_prop = decl_start + decl.property.len();
                if after_prop < ctx.source.len() {
                    let rest = ctx.source[after_prop..].trim_start();
                    if !rest.starts_with(':') {
                        continue;
                    }
                }
            }

            let has_empty = has_empty_line_before(ctx.source, decl_start);

            // Check if this is first-nested (first content after the opening `{`).
            let is_first = is_first_in_block_by_source(ctx.source, decl_start);

            // Check if this is a single-line block
            let is_single_line = is_single_line_block(ctx.source, rule);

            // Check if preceded by a comment (look at source before the declaration)
            let after_comment = is_after_comment(ctx.source, decl_start);

            // Check if preceded by a block (rule or at-rule ending with `}`)
            let after_block = is_after_block(ctx.source, decl_start);

            // Check if preceded by another standard (non-custom) declaration.
            // Use source-based analysis to correctly handle $var lines and
            // other non-declaration content.
            let after_declaration = is_after_declaration(ctx.source, decl_start);

            // ignore: ["inside-single-line-block"]
            if opts.ignore_inside_single_line_block && is_single_line {
                continue;
            }

            // ignore: ["after-comment"]
            if opts.ignore_after_comment && after_comment {
                continue;
            }

            // ignore: ["after-declaration"]
            if opts.ignore_after_declaration && after_declaration {
                continue;
            }

            // ignore: ["first-nested"]
            if opts.ignore_first_nested && is_first {
                continue;
            }

            // Determine expectation
            let expects_empty = match opts.primary {
                PrimaryOption::Always => true,
                PrimaryOption::Never => false,
            };

            let mut expectation = expects_empty;

            // Apply exceptions — first match only (Stylelint behavior).
            // Only the first matching exception flips the expectation.
            let exception_matched = if opts.except_first_nested && is_first {
                true
            } else if opts.except_after_comment && after_comment {
                true
            } else if opts.except_after_declaration && after_declaration {
                true
            } else {
                opts.except_after_block && after_block
            };

            if exception_matched {
                expectation = !expectation;
            }

            // Report
            if expectation && !has_empty {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Expected empty line before declaration \"{}\"",
                            decl.property
                        ),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(decl.span.offset, decl.span.length)),
                );
            } else if !expectation && has_empty {
                diags.push(
                    Diagnostic::new(
                        self.name(),
                        format!(
                            "Unexpected empty line before declaration \"{}\"",
                            decl.property
                        ),
                    )
                    .severity(self.default_severity())
                    .span(Span::new(decl.span.offset, decl.span.length)),
                );
            }
        }
        diags
    }
}

/// Parsed options for declaration-empty-line-before.
struct Options {
    primary: PrimaryOption,
    except_first_nested: bool,
    except_after_comment: bool,
    except_after_declaration: bool,
    except_after_block: bool,
    ignore_after_comment: bool,
    ignore_after_declaration: bool,
    ignore_inside_single_line_block: bool,
    ignore_first_nested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PrimaryOption {
    Always,
    Never,
}

impl Options {
    fn from_ctx(ctx: &RuleContext) -> Self {
        let mut opts = Options {
            primary: PrimaryOption::Never,
            except_first_nested: false,
            except_after_comment: false,
            except_after_declaration: false,
            except_after_block: false,
            ignore_after_comment: false,
            ignore_after_declaration: false,
            ignore_inside_single_line_block: false,
            ignore_first_nested: false,
        };

        let Some(value) = ctx.options else {
            return opts;
        };

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
        "always" => PrimaryOption::Always,
        _ => PrimaryOption::Never,
    }
}

fn parse_secondary(opts: &mut Options, value: &serde_json::Value) {
    if let Some(except) = value.get("except").and_then(|v| v.as_array()) {
        for item in except {
            if let Some(s) = item.as_str() {
                match s {
                    "first-nested" => opts.except_first_nested = true,
                    "after-comment" => opts.except_after_comment = true,
                    "after-declaration" => opts.except_after_declaration = true,
                    "after-block" => opts.except_after_block = true,
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
                    "after-declaration" => opts.ignore_after_declaration = true,
                    "inside-single-line-block" => opts.ignore_inside_single_line_block = true,
                    "first-nested" => opts.ignore_first_nested = true,
                    _ => {}
                }
            }
        }
    }
}

/// Check if the text before a node has an empty line (a line containing
/// only whitespace).
fn has_empty_line_before(source: &str, offset: usize) -> bool {
    if offset == 0 || offset > source.len() {
        return false;
    }
    let before = &source[..offset];

    // Find the start of the current line (skip the indentation leading
    // up to the node).
    let last_newline = before.rfind('\n');
    let Some(nl_pos) = last_newline else {
        return false;
    };

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
    false
}

/// Check if the declaration is the first thing after an opening brace.
///
/// Handles SCSS `//` comments and block `/* */` comments that may appear
/// after `{` on the same line or between `{` and the declaration.
fn is_first_in_block_by_source(source: &str, offset: usize) -> bool {
    if offset == 0 || offset > source.len() {
        return false;
    }
    let before = &source[..offset];

    // Walk backwards through the source, skipping whitespace, comments
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
        // Check for end of a block comment `*/`
        if pos >= 2 && &before[pos - 2..pos] == "*/" {
            // Find the matching `/*`
            if let Some(open) = before[..pos - 2].rfind("/*") {
                pos = open;
                continue;
            }
            return false;
        }
        // Check for SCSS line comment: find start of this line
        let line_start = before[..pos].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let line = before[line_start..pos].trim();
        if line.starts_with("//") {
            // This is a line comment; skip past it
            pos = line_start;
            continue;
        }
        // If the line contains a `//` comment after a `{`, strip the comment
        if let Some(comment_pos) = line.find("//") {
            let before_comment = line[..comment_pos].trim();
            if before_comment.ends_with('{') {
                return true;
            }
        }
        // Not a comment — check if it's `{`
        return bytes[pos - 1] == b'{';
    }
}

/// Check if the declaration is preceded by a comment in the source.
///
/// Stylelint considers a declaration to be "after-comment" when the
/// previous line (before the declaration's line) ends with a comment.
/// This includes:
/// - A standalone comment on its own line: `/* comment */\n decl`
/// - A trailing inline comment: `color: pink; /* comment */\n decl`
/// - SCSS line comments on their own line: `// comment\n decl`
///
/// NOT considered after-comment:
/// - Comment on the same line as the declaration: `/* comment */ decl`
/// - Comment on the opening brace line when decl is first-nested:
///   `a {/* comment */\n decl` (this is first-nested, not after-comment)
fn is_after_comment(source: &str, offset: usize) -> bool {
    if offset < 2 || offset > source.len() {
        return false;
    }
    let before = &source[..offset];

    // The declaration must be on a different line from the comment.
    // Find the start of the declaration's line.
    let decl_line_start = before.rfind('\n').map(|p| p + 1).unwrap_or(0);

    // Look at lines BEFORE the declaration's line.
    let before_decl_line = &before[..decl_line_start];

    // Walk backwards through previous lines to find the last meaningful line.
    for line in before_decl_line.lines().rev() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        // If the line ends with `*/`, it's after a block comment
        if stripped.ends_with("*/") {
            // But not if the line also starts with `{` (opening brace line with comment)
            // e.g. `a {/* comment */` -> not after-comment
            let before_comment = if let Some(open) = stripped.rfind("/*") {
                stripped[..open].trim()
            } else {
                ""
            };
            if before_comment.ends_with('{') {
                return false;
            }
            return true;
        }
        // If the line starts with `//`, it's a SCSS line comment
        if stripped.starts_with("//") {
            return true;
        }
        // If the line contains `//`, it has a trailing SCSS comment
        if stripped.contains("//") {
            return true;
        }
        return false;
    }
    false
}

/// Check if preceded by a block (a rule or at-rule ending with `}`).
fn is_after_block(source: &str, offset: usize) -> bool {
    if offset == 0 || offset > source.len() {
        return false;
    }
    let before = &source[..offset];

    // Walk backwards past whitespace to find the previous meaningful content
    for line in before.lines().rev() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        return stripped.ends_with('}');
    }
    false
}

/// Check if preceded by a standard declaration (not custom property, not $var).
///
/// Walks backwards through the source to find the previous non-empty, non-comment
/// line and checks if it looks like a standard CSS declaration (contains `:` and
/// doesn't start with `--`, `$`, or `@`).
fn is_after_declaration(source: &str, offset: usize) -> bool {
    if offset == 0 || offset > source.len() {
        return false;
    }
    let before = &source[..offset];

    // Walk backwards to find the previous meaningful line
    for line in before.lines().rev() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        // Skip comments
        if stripped.starts_with("//") || stripped.starts_with("/*") || stripped.ends_with("*/") {
            // If it's a trailing inline comment like `color: red; /* comment */`
            // we need to check if there's a declaration before the comment
            if let Some(comment_start) = stripped.find("/*") {
                let before_comment = stripped[..comment_start].trim();
                if !before_comment.is_empty() {
                    return is_declaration_like(before_comment);
                }
            }
            continue;
        }
        // If the line ends with `}`, it's a block, not a declaration
        if stripped.ends_with('}') {
            return false;
        }
        // If the line ends with `{`, it's a selector/at-rule
        if stripped.ends_with('{') {
            return false;
        }
        return is_declaration_like(stripped);
    }
    false
}

/// Check if a source line looks like a standard CSS declaration.
fn is_declaration_like(line: &str) -> bool {
    // Must contain a `:` to be a declaration
    if !line.contains(':') {
        return false;
    }
    // Custom properties are not "standard declarations"
    if line.starts_with("--") {
        return false;
    }
    // SCSS variables are not standard declarations
    if line.starts_with('$') {
        return false;
    }
    // At-rules are not declarations
    if line.starts_with('@') {
        return false;
    }
    true
}

/// Check if the style rule is a single-line block.
fn is_single_line_block(source: &str, rule: &gale_css_parser::StyleRule) -> bool {
    let start = rule.span.offset;
    let end = (start + rule.span.length).min(source.len());
    if start >= source.len() {
        return false;
    }
    !source[start..end].contains('\n')
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
    fn never_mode_reports_empty_line_before_declaration() {
        // Default is "never" mode
        let src = "a {\n  color: red;\n\n  display: block;\n}";
        let display_offset = src.find("display").unwrap();
        let display_len = "display: block;".len();
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(src.find("color").unwrap(), "color: red;".len()),
                    important: false,
                },
                Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(display_offset, display_len),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, src.len()),
        });
        let d = DeclarationEmptyLineBefore.check(&node, &make_ctx(src));
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("display"));
    }

    #[test]
    fn never_mode_allows_no_empty_line() {
        let src = "a {\n  color: red;\n  display: block;\n}";
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(src.find("color").unwrap(), "color: red;".len()),
                    important: false,
                },
                Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(src.find("display").unwrap(), "display: block;".len()),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, src.len()),
        });
        let d = DeclarationEmptyLineBefore.check(&node, &make_ctx(src));
        assert!(d.is_empty());
    }

    #[test]
    fn always_mode_with_ignore_after_declaration() {
        // Carbon config: ["always", { except: ["after-comment", "first-nested"], ignore: ["after-declaration"] }]
        let opts = serde_json::json!(["always", {"except": ["after-comment", "first-nested"], "ignore": ["after-declaration"]}]);
        let src = "a {\n  color: red;\n  display: block;\n}";
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(src.find("color").unwrap(), "color: red;".len()),
                    important: false,
                },
                Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(src.find("display").unwrap(), "display: block;".len()),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, src.len()),
        });
        let d = DeclarationEmptyLineBefore.check(&node, &make_ctx_with_options(src, &opts));
        // First decl: except first-nested flips "always" to "never", so no empty line needed — OK
        // Second decl: ignore after-declaration skips it entirely — OK
        assert!(d.is_empty(), "Expected no diagnostics with Carbon-like config, got {:?}", d.iter().map(|d| &d.message).collect::<Vec<_>>());
    }

    #[test]
    fn always_mode_reports_missing_empty_line() {
        let opts = serde_json::json!(["always"]);
        let src = "a {\n  color: red;\n  display: block;\n}";
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(src.find("color").unwrap(), "color: red;".len()),
                    important: false,
                },
                Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(src.find("display").unwrap(), "display: block;".len()),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, src.len()),
        });
        let d = DeclarationEmptyLineBefore.check(&node, &make_ctx_with_options(src, &opts));
        // Both should be reported: first (after {) and second (after declaration)
        assert_eq!(d.len(), 2, "Expected 2 diagnostics, got {}", d.len());
    }

    #[test]
    fn always_mode_except_first_nested() {
        let opts = serde_json::json!(["always", {"except": ["first-nested"]}]);
        let src = "a {\n  color: red;\n\n  display: block;\n}";
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(src.find("color").unwrap(), "color: red;".len()),
                    important: false,
                },
                Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(src.find("display").unwrap(), "display: block;".len()),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, src.len()),
        });
        let d = DeclarationEmptyLineBefore.check(&node, &make_ctx_with_options(src, &opts));
        // First decl: except first-nested flips "always" -> "never", no empty line, OK
        // Second decl: "always" mode, has empty line before, OK
        assert!(d.is_empty(), "Expected no diagnostics, got {:?}", d.iter().map(|d| &d.message).collect::<Vec<_>>());
    }

    #[test]
    fn ignore_inside_single_line_block() {
        let opts = serde_json::json!(["always", {"ignore": ["inside-single-line-block"]}]);
        let src = "a { color: red; display: block; }";
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![
                Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(src.find("color").unwrap(), "color: red;".len()),
                    important: false,
                },
                Declaration {
                    property: "display".to_string(),
                    value: "block".to_string(),
                    span: ParserSpan::new(src.find("display").unwrap(), "display: block;".len()),
                    important: false,
                },
            ],
            children: vec![],
            span: ParserSpan::new(0, src.len()),
        });
        let d = DeclarationEmptyLineBefore.check(&node, &make_ctx_with_options(src, &opts));
        assert!(d.is_empty(), "Single-line block should be ignored");
    }

    #[test]
    fn test_is_first_in_block_by_source_with_block_comment() {
        let src = "a { /* comment */\n top: 15px;\n}";
        let top_offset = src.find("top").unwrap();
        assert_eq!(top_offset, 19);
        assert!(
            is_first_in_block_by_source(src, top_offset),
            "top should be first-nested after {{ /* comment */"
        );
    }

    #[test]
    fn test_is_first_in_block_by_source_simple() {
        let src = "a {\n top: 15px;\n}";
        let top_offset = src.find("top").unwrap();
        assert!(
            is_first_in_block_by_source(src, top_offset),
            "top should be first-nested after {{"
        );
    }
}
