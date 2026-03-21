use lightningcss::printer::PrinterOptions;
use lightningcss::rules::CssRule as LcssRule;
use lightningcss::stylesheet::{ParserFlags, ParserOptions, StyleSheet};
use lightningcss::traits::ToCss;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Syntax detection
// ---------------------------------------------------------------------------

/// The kind of CSS dialect we are parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Syntax {
    Css,
    Scss,
    Less,
    Sass,
}

impl Default for Syntax {
    fn default() -> Self {
        Self::Css
    }
}

/// Infer the [`Syntax`] from a file extension.
///
/// Falls back to [`Syntax::Css`] for unrecognised extensions.
pub fn detect_syntax(file_path: &str) -> Syntax {
    match file_path.rsplit('.').next() {
        Some(ext) => match ext.to_ascii_lowercase().as_str() {
            "scss" => Syntax::Scss,
            "less" => Syntax::Less,
            "sass" => Syntax::Sass,
            _ => Syntax::Css,
        },
        None => Syntax::Css,
    }
}

// ---------------------------------------------------------------------------
// Simplified AST types (owned, lifetime-free)
// ---------------------------------------------------------------------------

/// A source span with byte offsets into the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    /// Byte offset from the start of the source.
    pub offset: usize,
    /// Length in bytes (0 when unknown).
    pub length: usize,
}

impl Span {
    pub fn new(offset: usize, length: usize) -> Self {
        Self { offset, length }
    }

    pub fn end(&self) -> usize {
        self.offset + self.length
    }

    fn empty() -> Self {
        Self {
            offset: 0,
            length: 0,
        }
    }
}

/// A CSS declaration (`property: value`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Declaration {
    pub property: String,
    pub value: String,
    pub span: Span,
    pub important: bool,
}

/// A CSS style rule (selector + declarations + optional nested children).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleRule {
    pub selector: String,
    pub declarations: Vec<Declaration>,
    pub span: Span,
    pub children: Vec<StyleRule>,
}

/// A CSS at-rule (`@media`, `@keyframes`, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtRule {
    pub name: String,
    pub params: String,
    pub span: Span,
    pub children: Vec<CssNode>,
}

/// A CSS comment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub text: String,
    pub span: Span,
}

/// A node in the simplified CSS tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CssNode {
    Style(StyleRule),
    AtRule(AtRule),
    Comment(Comment),
    Declaration(Declaration),
}

impl CssNode {
    /// Returns the span of this node.
    pub fn span(&self) -> Span {
        match self {
            CssNode::Style(r) => r.span,
            CssNode::AtRule(r) => r.span,
            CssNode::Comment(c) => c.span,
            CssNode::Declaration(d) => d.span,
        }
    }

    /// Returns the child nodes of this node.
    pub fn children(&self) -> Vec<&CssNode> {
        match self {
            CssNode::Style(_) => {
                // StyleRule children are nested StyleRules, not CssNodes.
                // The runner handles recursion into StyleRule.children directly.
                Vec::new()
            }
            CssNode::AtRule(at_rule) => at_rule.children.iter().collect(),
            CssNode::Comment(_) | CssNode::Declaration(_) => Vec::new(),
        }
    }
}

/// The result of parsing a CSS source string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParseResult {
    pub nodes: Vec<CssNode>,
    pub syntax: Syntax,
    pub source: String,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("CSS parse error: {message}")]
    Css { message: String },

    #[error("{syntax:?} parsing is not yet implemented (TODO)")]
    UnsupportedSyntax { syntax: Syntax },
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse a CSS (or SCSS/Less/Sass) source string into a [`ParseResult`].
///
/// Currently only [`Syntax::Css`] is fully supported via *lightningcss*.
/// SCSS, Less and Sass will return [`ParseError::UnsupportedSyntax`].
pub fn parse(source: &str, syntax: Syntax) -> Result<ParseResult, ParseError> {
    match syntax {
        Syntax::Css => parse_css(source),
        other => Err(ParseError::UnsupportedSyntax { syntax: other }),
    }
}

// ---------------------------------------------------------------------------
// Helpers – create fresh default PrinterOptions each time we need one.
//
// PrinterOptions contains an `Option<&mut SourceMap>` so it is neither
// `Copy` nor cheaply cloneable. Constructing a default is trivial.
// ---------------------------------------------------------------------------

fn po() -> PrinterOptions<'static> {
    PrinterOptions::default()
}

// ---------------------------------------------------------------------------
// lightningcss → simplified AST conversion
// ---------------------------------------------------------------------------

fn parse_css(source: &str) -> Result<ParseResult, ParseError> {
    let opts = ParserOptions {
        flags: ParserFlags::NESTING,
        error_recovery: true,
        ..ParserOptions::default()
    };

    let stylesheet =
        StyleSheet::parse(source, opts).map_err(|err| ParseError::Css {
            message: err.to_string(),
        })?;

    let nodes = convert_rules(&stylesheet.rules.0, source);

    Ok(ParseResult {
        nodes,
        syntax: Syntax::Css,
        source: source.to_owned(),
    })
}

/// Convert a list of lightningcss rules into our [`CssNode`] list.
fn convert_rules(rules: &[LcssRule], source: &str) -> Vec<CssNode> {
    let mut nodes = Vec::with_capacity(rules.len());

    for rule in rules {
        match rule {
            LcssRule::Style(style) => {
                nodes.push(CssNode::Style(convert_style_rule(style, source)));
            }

            LcssRule::Media(media) => {
                let params = media.query.to_css_string(po()).unwrap_or_default();
                let children = convert_rules(&media.rules.0, source);
                nodes.push(CssNode::AtRule(AtRule {
                    name: "media".into(),
                    params,
                    span: loc_to_span(media.loc, source),
                    children,
                }));
            }

            LcssRule::Supports(supports) => {
                let params = supports.condition.to_css_string(po()).unwrap_or_default();
                let children = convert_rules(&supports.rules.0, source);
                nodes.push(CssNode::AtRule(AtRule {
                    name: "supports".into(),
                    params,
                    span: loc_to_span(supports.loc, source),
                    children,
                }));
            }

            LcssRule::Keyframes(kf) => {
                let params = kf.name.to_css_string(po()).unwrap_or_default();
                nodes.push(CssNode::AtRule(AtRule {
                    name: "keyframes".into(),
                    params,
                    span: loc_to_span(kf.loc, source),
                    children: Vec::new(),
                }));
            }

            LcssRule::FontFace(ff) => {
                // FontFaceProperty is its own enum (not Property).
                // Serialize each via ToCss then split on `:`.
                let mut children = Vec::new();
                for prop in &ff.properties {
                    let css = prop.to_css_string(po()).unwrap_or_default();
                    if let Some((name, value)) = css.split_once(':') {
                        children.push(CssNode::Declaration(Declaration {
                            property: name.trim().to_owned(),
                            value: value.trim().to_owned(),
                            span: Span::empty(),
                            important: false,
                        }));
                    }
                }
                nodes.push(CssNode::AtRule(AtRule {
                    name: "font-face".into(),
                    params: String::new(),
                    span: loc_to_span(ff.loc, source),
                    children,
                }));
            }

            LcssRule::Import(import) => {
                nodes.push(CssNode::AtRule(AtRule {
                    name: "import".into(),
                    params: import.url.as_ref().to_owned(),
                    span: loc_to_span(import.loc, source),
                    children: Vec::new(),
                }));
            }

            LcssRule::Namespace(ns) => {
                nodes.push(CssNode::AtRule(AtRule {
                    name: "namespace".into(),
                    params: ns.url.as_ref().to_owned(),
                    span: loc_to_span(ns.loc, source),
                    children: Vec::new(),
                }));
            }

            LcssRule::Container(container) => {
                let params = container
                    .name
                    .as_ref()
                    .map(|n| n.to_css_string(po()).unwrap_or_default())
                    .unwrap_or_default();
                let children = convert_rules(&container.rules.0, source);
                nodes.push(CssNode::AtRule(AtRule {
                    name: "container".into(),
                    params,
                    span: loc_to_span(container.loc, source),
                    children,
                }));
            }

            LcssRule::LayerBlock(layer) => {
                let params = layer
                    .name
                    .as_ref()
                    .map(|n| {
                        n.0.iter()
                            .map(|s| s.as_ref())
                            .collect::<Vec<_>>()
                            .join(".")
                    })
                    .unwrap_or_default();
                let children = convert_rules(&layer.rules.0, source);
                nodes.push(CssNode::AtRule(AtRule {
                    name: "layer".into(),
                    params,
                    span: loc_to_span(layer.loc, source),
                    children,
                }));
            }

            LcssRule::LayerStatement(layer) => {
                let params = layer
                    .names
                    .iter()
                    .map(|n| {
                        n.0.iter()
                            .map(|s| s.as_ref())
                            .collect::<Vec<_>>()
                            .join(".")
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                nodes.push(CssNode::AtRule(AtRule {
                    name: "layer".into(),
                    params,
                    span: loc_to_span(layer.loc, source),
                    children: Vec::new(),
                }));
            }

            LcssRule::Scope(scope) => {
                let children = convert_rules(&scope.rules.0, source);
                nodes.push(CssNode::AtRule(AtRule {
                    name: "scope".into(),
                    params: String::new(),
                    span: loc_to_span(scope.loc, source),
                    children,
                }));
            }

            LcssRule::StartingStyle(ss) => {
                let children = convert_rules(&ss.rules.0, source);
                nodes.push(CssNode::AtRule(AtRule {
                    name: "starting-style".into(),
                    params: String::new(),
                    span: loc_to_span(ss.loc, source),
                    children,
                }));
            }

            LcssRule::Nesting(nesting) => {
                nodes.push(CssNode::Style(convert_style_rule(&nesting.style, source)));
            }

            LcssRule::NestedDeclarations(nested_decls) => {
                for decl in &nested_decls.declarations.declarations {
                    nodes.push(CssNode::Declaration(convert_property(decl, false, source)));
                }
                for decl in &nested_decls.declarations.important_declarations {
                    nodes.push(CssNode::Declaration(convert_property(decl, true, source)));
                }
            }

            LcssRule::Page(page) => {
                nodes.push(CssNode::AtRule(AtRule {
                    name: "page".into(),
                    params: String::new(),
                    span: loc_to_span(page.loc, source),
                    children: Vec::new(),
                }));
            }

            LcssRule::Property(prop) => {
                let params = prop.name.to_css_string(po()).unwrap_or_default();
                nodes.push(CssNode::AtRule(AtRule {
                    name: "property".into(),
                    params,
                    span: loc_to_span(prop.loc, source),
                    children: Vec::new(),
                }));
            }

            LcssRule::CounterStyle(cs) => {
                let params = cs.name.to_css_string(po()).unwrap_or_default();
                nodes.push(CssNode::AtRule(AtRule {
                    name: "counter-style".into(),
                    params,
                    span: loc_to_span(cs.loc, source),
                    children: Vec::new(),
                }));
            }

            LcssRule::Unknown(unknown) => {
                let name = unknown.name.as_ref().to_owned();
                // TokenList::to_css is pub(crate) in lightningcss, so we
                // serialize the whole rule and strip the `@name` prefix to
                // extract the prelude.
                let full = unknown.to_css_string(po()).unwrap_or_default();
                let params = full
                    .strip_prefix(&format!("@{name}"))
                    .map(|rest| rest.trim().trim_end_matches(';').trim().to_owned())
                    .unwrap_or_default();
                nodes.push(CssNode::AtRule(AtRule {
                    name,
                    params,
                    span: loc_to_span(unknown.loc, source),
                    children: Vec::new(),
                }));
            }

            // Remaining variants we don't model yet (e.g. MozDocument,
            // FontPaletteValues, ViewTransition, etc.) are silently skipped.
            _ => {}
        }
    }

    nodes
}

fn convert_style_rule(
    style: &lightningcss::rules::style::StyleRule,
    source: &str,
) -> StyleRule {
    let selector = style.selectors.to_css_string(po()).unwrap_or_default();

    let mut declarations = Vec::new();
    for decl in &style.declarations.declarations {
        declarations.push(convert_property(decl, false, source));
    }
    for decl in &style.declarations.important_declarations {
        declarations.push(convert_property(decl, true, source));
    }

    // Nested rules: extract only nested style rules as direct children.
    let mut children = Vec::new();
    for rule in &style.rules.0 {
        if let LcssRule::Style(nested_style) = rule {
            children.push(convert_style_rule(nested_style, source));
        }
    }

    StyleRule {
        selector,
        declarations,
        span: loc_to_span(style.loc, source),
        children,
    }
}

fn convert_property(
    prop: &lightningcss::properties::Property,
    important: bool,
    source: &str,
) -> Declaration {
    let property_name = prop.property_id().name().to_owned();
    let value = prop.value_to_css_string(po()).unwrap_or_default();

    // Try to find the property in the source text for a proper byte offset.
    // Properties don't carry their own Location in lightningcss, so we use
    // Span::empty() as a fallback.
    let _ = source; // acknowledge the parameter; future improvement can search source

    Declaration {
        property: property_name,
        value,
        span: Span::empty(),
        important,
    }
}

/// Convert a 0-indexed line and 1-based column (as reported by lightningcss)
/// into a byte offset within `source`.
///
/// Returns 0 if the line/column is out of range.
fn line_col_to_byte_offset(source: &str, line: u32, column: u32) -> usize {
    let mut current_line: u32 = 0;
    let mut line_start: usize = 0;

    for (i, ch) in source.char_indices() {
        if current_line == line {
            line_start = i;
            break;
        }
        if ch == '\n' {
            current_line += 1;
            line_start = i + 1;
        }
    }

    if current_line < line {
        // line is beyond end of source
        return source.len();
    }

    // column is 1-based, measured in UTF-16 code units.
    // Walk from line_start counting UTF-16 code units.
    let mut col: u32 = 1;
    for (i, ch) in source[line_start..].char_indices() {
        if col >= column {
            return line_start + i;
        }
        col += ch.len_utf16() as u32;
    }

    // column points past end of line
    source.len().min(line_start + source[line_start..].len())
}

/// Map a lightningcss `Location` (line/column) into our `Span` as a byte offset.
fn loc_to_span(loc: lightningcss::rules::Location, source: &str) -> Span {
    let offset = line_col_to_byte_offset(source, loc.line, loc.column);
    Span {
        offset,
        length: 0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_syntax() {
        assert_eq!(detect_syntax("foo.css"), Syntax::Css);
        assert_eq!(detect_syntax("bar.scss"), Syntax::Scss);
        assert_eq!(detect_syntax("baz.less"), Syntax::Less);
        assert_eq!(detect_syntax("qux.sass"), Syntax::Sass);
        assert_eq!(detect_syntax("unknown.txt"), Syntax::Css);
        assert_eq!(detect_syntax("noext"), Syntax::Css);
    }

    #[test]
    fn test_parse_simple_css() {
        let css = r#"
            .foo {
                color: red;
                display: block;
            }
        "#;
        let result = parse(css, Syntax::Css).expect("should parse");
        assert_eq!(result.syntax, Syntax::Css);
        assert_eq!(result.nodes.len(), 1);

        if let CssNode::Style(ref rule) = result.nodes[0] {
            assert_eq!(rule.selector, ".foo");
            assert_eq!(rule.declarations.len(), 2);
            assert_eq!(rule.declarations[0].property, "color");
            assert_eq!(rule.declarations[0].value, "red");
            assert!(!rule.declarations[0].important);
            assert_eq!(rule.declarations[1].property, "display");
            assert_eq!(rule.declarations[1].value, "block");
        } else {
            panic!("expected a Style node");
        }
    }

    #[test]
    fn test_parse_important() {
        let css = ".bar { margin: 0 !important; }";
        let result = parse(css, Syntax::Css).unwrap();
        assert_eq!(result.nodes.len(), 1);

        if let CssNode::Style(ref rule) = result.nodes[0] {
            assert_eq!(rule.declarations.len(), 1);
            assert!(rule.declarations[0].important);
        } else {
            panic!("expected a Style node");
        }
    }

    #[test]
    fn test_parse_media_rule() {
        let css = "@media (min-width: 768px) { .foo { color: blue; } }";
        let result = parse(css, Syntax::Css).unwrap();
        assert_eq!(result.nodes.len(), 1);

        if let CssNode::AtRule(ref at_rule) = result.nodes[0] {
            assert_eq!(at_rule.name, "media");
            assert!(!at_rule.params.is_empty());
            assert_eq!(at_rule.children.len(), 1);
        } else {
            panic!("expected an AtRule node");
        }
    }

    #[test]
    fn test_unsupported_syntax() {
        let err = parse("$foo: red;", Syntax::Scss).unwrap_err();
        assert!(matches!(err, ParseError::UnsupportedSyntax { .. }));
    }

    #[test]
    fn test_parse_nested_rules() {
        let css = r#"
            .parent {
                color: red;
                .child {
                    color: blue;
                }
            }
        "#;
        let result = parse(css, Syntax::Css).unwrap();
        assert_eq!(result.nodes.len(), 1);

        if let CssNode::Style(ref rule) = result.nodes[0] {
            assert_eq!(rule.selector, ".parent");
            assert!(!rule.children.is_empty());
            // lightningcss includes the `&` nesting selector prefix.
            assert!(
                rule.children[0].selector.contains(".child"),
                "expected nested selector to contain '.child', got: {}",
                rule.children[0].selector,
            );
        } else {
            panic!("expected a Style node");
        }
    }

    #[test]
    fn test_parse_import() {
        let css = r#"@import "reset.css";"#;
        let result = parse(css, Syntax::Css).unwrap();
        assert_eq!(result.nodes.len(), 1);

        if let CssNode::AtRule(ref at_rule) = result.nodes[0] {
            assert_eq!(at_rule.name, "import");
            assert_eq!(at_rule.params, "reset.css");
        } else {
            panic!("expected an AtRule node");
        }
    }

    #[test]
    fn test_parse_keyframes() {
        let css = "@keyframes fade { from { opacity: 0; } to { opacity: 1; } }";
        let result = parse(css, Syntax::Css).unwrap();
        assert_eq!(result.nodes.len(), 1);

        if let CssNode::AtRule(ref at_rule) = result.nodes[0] {
            assert_eq!(at_rule.name, "keyframes");
            assert_eq!(at_rule.params, "fade");
        } else {
            panic!("expected an AtRule node");
        }
    }

    #[test]
    fn test_parse_multiple_rules() {
        let css = r#"
            .a { color: red; }
            .b { color: blue; }
        "#;
        let result = parse(css, Syntax::Css).unwrap();
        assert_eq!(result.nodes.len(), 2);
    }

    #[test]
    fn test_default_syntax_is_css() {
        assert_eq!(Syntax::default(), Syntax::Css);
    }

    #[test]
    fn test_line_col_to_byte_offset() {
        let src = "abc\ndef\nghi";
        // line 0, col 1 => byte 0 ('a')
        assert_eq!(line_col_to_byte_offset(src, 0, 1), 0);
        // line 0, col 3 => byte 2 ('c')
        assert_eq!(line_col_to_byte_offset(src, 0, 3), 2);
        // line 1, col 1 => byte 4 ('d')
        assert_eq!(line_col_to_byte_offset(src, 1, 1), 4);
        // line 1, col 2 => byte 5 ('e')
        assert_eq!(line_col_to_byte_offset(src, 1, 2), 5);
        // line 2, col 1 => byte 8 ('g')
        assert_eq!(line_col_to_byte_offset(src, 2, 1), 8);
    }

    #[test]
    fn test_span_is_byte_offset() {
        // Parse a simple rule and verify span.offset is a valid byte offset.
        let css = "a { }\n.b { }";
        let result = parse(css, Syntax::Css).unwrap();
        assert_eq!(result.nodes.len(), 2);

        // "a" starts at byte 0
        if let CssNode::Style(ref rule) = result.nodes[0] {
            assert_eq!(rule.span.offset, 0);
            assert_eq!(&css[rule.span.offset..rule.span.offset + 1], "a");
        } else {
            panic!("expected Style node");
        }

        // ".b" starts at byte 6
        if let CssNode::Style(ref rule) = result.nodes[1] {
            assert_eq!(rule.span.offset, 6);
            assert_eq!(&css[rule.span.offset..rule.span.offset + 2], ".b");
        } else {
            panic!("expected Style node");
        }
    }
}
