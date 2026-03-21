use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow qualifying a selector by type (e.g. `a.foo`, `div#bar`).
///
/// Equivalent to Stylelint's `selector-no-qualifying-type` rule.
pub struct SelectorNoQualifyingType;

impl Rule for SelectorNoQualifyingType {
    fn name(&self) -> &'static str {
        "selector-no-qualifying-type"
    }

    fn description(&self) -> &'static str {
        "Disallow qualifying a selector by type"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, nodes: &[CssNode], ctx: &RuleContext) -> Vec<Diagnostic> {
        let ignore_class = has_ignore_option(ctx, "class");
        let ignore_attribute = has_ignore_option(ctx, "attribute");
        let ignore_id = has_ignore_option(ctx, "id");

        let is_scss = matches!(
            ctx.syntax,
            gale_css_parser::Syntax::Scss | gale_css_parser::Syntax::Sass
        );

        let mut diags = Vec::new();
        walk_nodes(
            self,
            nodes,
            ctx,
            is_scss,
            false, // ancestor_has_interpolation
            ignore_class,
            ignore_attribute,
            ignore_id,
            &mut diags,
        );
        diags
    }
}

/// Recursively walk nodes, tracking whether any ancestor selector contains
/// SCSS interpolation. Stylelint skips rules whose resolved (flattened)
/// selector is non-standard, which includes any rule nested inside a parent
/// whose selector contains `#{...}`.
fn walk_nodes(
    rule_impl: &SelectorNoQualifyingType,
    nodes: &[CssNode],
    ctx: &RuleContext,
    is_scss: bool,
    ancestor_has_interpolation: bool,
    ignore_class: bool,
    ignore_attribute: bool,
    ignore_id: bool,
    diags: &mut Vec<Diagnostic>,
) {
    for node in nodes {
        match node {
            CssNode::Style(style) => {
                let self_has_interpolation =
                    is_scss && style.selector.contains("#{");
                let any_interpolation =
                    ancestor_has_interpolation || self_has_interpolation;

                // Only check this selector if neither it nor any ancestor
                // contains SCSS interpolation (matching Stylelint's
                // `isStandardSyntaxRule` skip).
                if !any_interpolation {
                    for sel in style.selector.split(',') {
                        let sel = sel.trim();
                        if has_qualifying_type_with_options(
                            sel,
                            ignore_class,
                            ignore_attribute,
                            ignore_id,
                        ) {
                            diags.push(
                                Diagnostic::new(
                                    rule_impl.name(),
                                    format!(
                                        "Unexpected qualifying type selector in \"{sel}\""
                                    ),
                                )
                                .severity(rule_impl.default_severity())
                                .span(Span::new(style.span.offset, style.span.length)),
                            );
                        }
                    }
                }

                // Recurse into children, propagating interpolation flag.
                walk_style_children(
                    rule_impl,
                    style,
                    ctx,
                    is_scss,
                    any_interpolation,
                    ignore_class,
                    ignore_attribute,
                    ignore_id,
                    diags,
                );
            }
            CssNode::AtRule(at_rule) => {
                walk_nodes(
                    rule_impl,
                    &at_rule.children,
                    ctx,
                    is_scss,
                    ancestor_has_interpolation,
                    ignore_class,
                    ignore_attribute,
                    ignore_id,
                    diags,
                );
            }
            _ => {}
        }
    }
}

fn walk_style_children(
    rule_impl: &SelectorNoQualifyingType,
    style: &gale_css_parser::StyleRule,
    ctx: &RuleContext,
    is_scss: bool,
    ancestor_has_interpolation: bool,
    ignore_class: bool,
    ignore_attribute: bool,
    ignore_id: bool,
    diags: &mut Vec<Diagnostic>,
) {
    for child in &style.children {
        let self_has_interpolation = is_scss && child.selector.contains("#{");
        let any_interpolation = ancestor_has_interpolation || self_has_interpolation;

        if !any_interpolation {
            for sel in child.selector.split(',') {
                let sel = sel.trim();
                if has_qualifying_type_with_options(
                    sel,
                    ignore_class,
                    ignore_attribute,
                    ignore_id,
                ) {
                    diags.push(
                        Diagnostic::new(
                            rule_impl.name(),
                            format!("Unexpected qualifying type selector in \"{sel}\""),
                        )
                        .severity(rule_impl.default_severity())
                        .span(Span::new(child.span.offset, child.span.length)),
                    );
                }
            }
        }

        walk_style_children(
            rule_impl,
            child,
            ctx,
            is_scss,
            any_interpolation,
            ignore_class,
            ignore_attribute,
            ignore_id,
            diags,
        );
    }
}

/// Check if the `ignore` secondary option contains a given value.
fn has_ignore_option(ctx: &RuleContext, value: &str) -> bool {
    ctx.secondary_options()
        .and_then(|v| v.get("ignore"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().any(|v| v.as_str() == Some(value)))
        .unwrap_or(false)
}

/// Check if a simple selector has a type selector immediately followed by a
/// class (`.`), ID (`#`), or attribute (`[`) selector, respecting ignore options.
fn has_qualifying_type_with_options(
    selector: &str,
    ignore_class: bool,
    ignore_attribute: bool,
    ignore_id: bool,
) -> bool {
    let chars: Vec<char> = selector.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Skip whitespace (combinators)
        if chars[i].is_ascii_whitespace() || chars[i] == '>' || chars[i] == '+' || chars[i] == '~' {
            i += 1;
            continue;
        }

        // Check if we're at the start of a type selector
        if is_type_selector_start(&chars, i) {
            let start = i;
            while i < len && is_ident_char(chars[i]) {
                i += 1;
            }
            if i > start && i < len {
                if chars[i] == '.' && !ignore_class {
                    return true;
                }
                if chars[i] == '#' && !ignore_id {
                    return true;
                }
                if chars[i] == '[' && !ignore_attribute {
                    return true;
                }
            }
            continue;
        }

        // Skip class selectors (.foo)
        if chars[i] == '.' {
            i += 1;
            while i < len && is_ident_char(chars[i]) {
                i += 1;
            }
            continue;
        }

        // Skip ID selectors (#foo)
        if chars[i] == '#' {
            i += 1;
            while i < len && is_ident_char(chars[i]) {
                i += 1;
            }
            continue;
        }

        // Skip pseudo-classes/pseudo-elements (:hover, ::before)
        if chars[i] == ':' {
            i += 1;
            if i < len && chars[i] == ':' {
                i += 1;
            }
            while i < len && is_ident_char(chars[i]) {
                i += 1;
            }
            if i < len && chars[i] == '(' {
                let mut depth = 1;
                i += 1;
                while i < len && depth > 0 {
                    if chars[i] == '(' {
                        depth += 1;
                    } else if chars[i] == ')' {
                        depth -= 1;
                    }
                    i += 1;
                }
            }
            continue;
        }

        // Skip attribute selectors [attr]
        if chars[i] == '[' {
            while i < len && chars[i] != ']' {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            continue;
        }

        // Skip * (universal selector)
        if chars[i] == '*' {
            i += 1;
            continue;
        }

        i += 1;
    }
    false
}

fn is_type_selector_start(chars: &[char], i: usize) -> bool {
    let ch = chars[i];
    if !ch.is_ascii_alphabetic() && ch.is_ascii() {
        return false;
    }
    // Must not be preceded by '.', '#', or ':' (which would mean it's part of a class/id/pseudo)
    if i > 0 {
        let prev = chars[i - 1];
        if prev == '.' || prev == '#' || prev == ':' {
            return false;
        }
    }
    true
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || !ch.is_ascii()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        }
    }

    fn style_with_selector(sel: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: sel.to_string(),
            declarations: vec![Declaration {
                property: "color".to_string(),
                value: "red".to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_type_with_class() {
        let node = style_with_selector("a.foo");
        let d = SelectorNoQualifyingType.check_root(&[node], &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("a.foo"));
    }

    #[test]
    fn reports_type_with_id() {
        let node = style_with_selector("div#bar");
        let d = SelectorNoQualifyingType.check_root(&[node], &ctx());
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn allows_class_only() {
        let node = style_with_selector(".foo");
        let d = SelectorNoQualifyingType.check_root(&[node], &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_type_only() {
        let node = style_with_selector("div");
        let d = SelectorNoQualifyingType.check_root(&[node], &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn skips_nested_in_scss_interpolation_parent() {
        // a.foo nested inside .#{$var} should be skipped in SCSS mode
        let scss_ctx = RuleContext {
            file_path: "t.scss",
            source: "",
            syntax: Syntax::Scss,
            options: None,
        };
        let parent = CssNode::Style(StyleRule {
            selector: ".#{$var}".to_string(),
            declarations: vec![],
            children: vec![StyleRule {
                selector: "a.foo".to_string(),
                declarations: vec![Declaration {
                    property: "color".to_string(),
                    value: "red".to_string(),
                    span: ParserSpan::new(0, 0),
                    important: false,
                }],
                children: vec![],
                span: ParserSpan::new(0, 0),
            }],
            span: ParserSpan::new(0, 0),
        });
        let d = SelectorNoQualifyingType.check_root(&[parent], &scss_ctx);
        assert!(
            d.is_empty(),
            "should skip rules nested inside SCSS interpolation parent"
        );
    }
}
