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

        // Build a combined list of children (declarations + nested style rules as comments)
        // so we can determine what precedes each declaration.
        // For this rule, we only look at declarations within the rule.
        // We need to check adjacent items: declarations and comments interleaved.
        //
        // The AST gives us rule.declarations (only declarations) but in the source
        // there may be comments between them. We'll use source analysis for context.

        for (i, decl) in rule.declarations.iter().enumerate() {
            // Stylelint's declaration-empty-line-before only checks standard
            // property declarations.  Custom properties (starting with `--`)
            // are handled by `custom-property-empty-line-before` instead.
            if decl.property.starts_with("--") {
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
            // Use source analysis rather than the declaration index, because
            // at-rules like @include may precede the first declaration.
            let is_first = is_first_in_block_by_source(ctx.source, decl_start);

            // Check if this is a single-line block
            let is_single_line = is_single_line_block(ctx.source, rule);

            // Check if preceded by a comment (look at source before the declaration)
            let after_comment = is_after_comment(ctx.source, decl_start);

            // Check if preceded by another standard (non-custom) declaration.
            // In Stylelint, `except: after-declaration` only considers standard
            // property declarations, NOT custom properties (those starting with `--`).
            // Find the nearest previous standard declaration.
            let prev_std_decl = rule.declarations[..i]
                .iter()
                .rev()
                .find(|d| !d.property.starts_with("--"));

            let after_declaration = if let Some(prev) = prev_std_decl {
                // Quick check: if a `}` appears between the previous standard
                // declaration and this one, there's a block in between.
                let prev_end = prev.span.offset + prev.span.length;
                let between = if prev_end < decl_start && decl_start <= ctx.source.len() {
                    &ctx.source[prev_end..decl_start]
                } else {
                    ""
                };
                !between.contains('}')
            } else {
                false
            };

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

            // Determine expectation
            let expects_empty = match opts.primary {
                PrimaryOption::Always => true,
                PrimaryOption::Never => false,
            };

            let mut expectation = expects_empty;

            // Apply exceptions (they flip the expectation)
            if opts.except_first_nested && is_first {
                expectation = !expectation;
            }

            if opts.except_after_comment && after_comment {
                expectation = !expectation;
            }

            if opts.except_after_declaration && after_declaration {
                expectation = !expectation;
            }

            // Report
            if expectation && !has_empty {
                // Only report if not first item with no preceding content
                // (first-nested without exception: for "always", first decl needs empty line
                //  but that's unusual — Stylelint checks source context)
                if is_first && is_first_in_block_by_source(ctx.source, decl_start) && !opts.except_first_nested {
                    // First declaration right after opening brace — in "always" mode
                    // Stylelint expects an empty line even here unless except first-nested
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
                } else if !is_first || !is_first_in_block_by_source(ctx.source, decl_start) {
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
                }
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
    ignore_after_comment: bool,
    ignore_after_declaration: bool,
    ignore_inside_single_line_block: bool,
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
            ignore_after_comment: false,
            ignore_after_declaration: false,
            ignore_inside_single_line_block: false,
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
/// Handles SCSS `//` comments that may appear after `{` on the same line,
/// e.g. `.foo { // comment\n  decl: val; }` — `decl` is first-nested.
fn is_first_in_block_by_source(source: &str, offset: usize) -> bool {
    if offset == 0 || offset > source.len() {
        return false;
    }
    let before = &source[..offset];

    // Walk backwards line by line. Skip blank lines and lines that are
    // only SCSS line comments.
    for line in before.lines().rev() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        // If the line is a pure SCSS comment (starts with //), skip it
        // and keep looking.
        if stripped.starts_with("//") {
            continue;
        }
        // Check if the line ends with `{` (possibly followed by a //
        // comment on the same line).
        let effective = if let Some(pos) = stripped.find("//") {
            stripped[..pos].trim()
        } else {
            stripped
        };
        return effective.ends_with('{');
    }
    false
}

/// Check if the declaration is preceded by a comment in the source.
fn is_after_comment(source: &str, offset: usize) -> bool {
    if offset < 2 || offset > source.len() {
        return false;
    }
    let before = &source[..offset];
    let trimmed = before.trim();
    // Check for block comment ending
    if trimmed.ends_with("*/") {
        return true;
    }
    // Check for SCSS line comment: find the last non-empty line before
    // this declaration and see if it starts with "//".
    for line in before.lines().rev() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        return stripped.starts_with("//");
    }
    false
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
}
