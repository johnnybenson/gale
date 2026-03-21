use gale_css_parser::{CssNode, Syntax};
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports when a CSS rule block has no declarations or nested children.
///
/// Equivalent to Stylelint's `block-no-empty` rule.
pub struct BlockNoEmpty;

impl Rule for BlockNoEmpty {
    fn name(&self) -> &'static str {
        "block-no-empty"
    }

    fn description(&self) -> &'static str {
        "Disallow empty blocks"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, context: &RuleContext) -> Vec<Diagnostic> {
        match node {
            CssNode::Style(rule) if rule.declarations.is_empty() && rule.children.is_empty() => {
                // In SCSS/Less/Sass, the AST may not capture all children
                // (e.g. `@include`, `$var` assignments). Fall back to checking
                // the source text between `{` and `}` for non-whitespace content.
                if matches!(context.syntax, Syntax::Scss | Syntax::Less | Syntax::Sass) {
                    let start = rule.span.offset;
                    let end = start + rule.span.length;
                    if end <= context.source.len() {
                        let block_src = &context.source[start..end];
                        if let Some(open) = block_src.find('{') {
                            let body = &block_src[open + 1..];
                            let body = body.strip_suffix('}').unwrap_or(body);
                            if !body.trim().is_empty() {
                                return vec![];
                            }
                        }
                    }
                }

                vec![
                    Diagnostic::new(self.name(), "Unexpected empty block")
                        .severity(self.default_severity())
                        .span(Span::new(rule.span.offset, rule.span.length)),
                ]
            }
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, StyleRule, Syntax};

    fn make_context() -> RuleContext<'static> {
        RuleContext {
            file_path: "test.css",
            source: "",
            syntax: Syntax::Css, options: None }
    }

    #[test]
    fn reports_empty_style_rule() {
        let rule = BlockNoEmpty;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![],
            children: vec![],
            span: ParserSpan::new(0, 5),
        });
        let diags = rule.check(&node, &make_context());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "Unexpected empty block");
    }

    #[test]
    fn ignores_non_empty_style_rule() {
        let rule = BlockNoEmpty;
        let node = CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "color".to_string(),
                value: "red".to_string(),
                span: ParserSpan::new(4, 10),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 20),
        });
        let diags = rule.check(&node, &make_context());
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_scss_block_with_include() {
        let rule = BlockNoEmpty;
        let source = ".btn { @include button-styles; }";
        let node = CssNode::Style(StyleRule {
            selector: ".btn".to_string(),
            declarations: vec![],
            children: vec![],
            span: ParserSpan::new(0, source.len()),
        });
        let ctx = RuleContext {
            file_path: "test.scss",
            source,
            syntax: Syntax::Scss, options: None };
        let diags = rule.check(&node, &ctx);
        assert!(diags.is_empty());
    }

    #[test]
    fn reports_truly_empty_scss_block() {
        let rule = BlockNoEmpty;
        let source = ".btn {  }";
        let node = CssNode::Style(StyleRule {
            selector: ".btn".to_string(),
            declarations: vec![],
            children: vec![],
            span: ParserSpan::new(0, source.len()),
        });
        let ctx = RuleContext {
            file_path: "test.scss",
            source,
            syntax: Syntax::Scss, options: None };
        let diags = rule.check(&node, &ctx);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn ignores_non_style_rule_nodes() {
        let rule = BlockNoEmpty;
        let node = CssNode::Comment(gale_css_parser::Comment {
            text: "/* hi */".to_string(),
            span: ParserSpan::new(0, 8),
            is_line: false,
        });
        let diags = rule.check(&node, &make_context());
        assert!(diags.is_empty());
    }
}
