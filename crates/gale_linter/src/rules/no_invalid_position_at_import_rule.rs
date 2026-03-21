use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports `@import` rules that appear after non-import statements.
///
/// `@import` rules must precede all other rules (except `@charset`, `@layer`,
/// and comments) to be valid CSS.
///
/// Equivalent to Stylelint's `no-invalid-position-at-import-rule` rule.
pub struct NoInvalidPositionAtImportRule;

impl Rule for NoInvalidPositionAtImportRule {
    fn name(&self) -> &'static str {
        "no-invalid-position-at-import-rule"
    }

    fn description(&self) -> &'static str {
        "Disallow invalid position @import rules"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_root(&self, nodes: &[CssNode], _context: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut seen_non_import = false;

        for node in nodes {
            match node {
                // Comments are always allowed anywhere.
                CssNode::Comment(_) => continue,

                CssNode::AtRule(at_rule) => {
                    let name = at_rule.name.as_str();
                    match name {
                        // These are allowed before @import.
                        "charset" | "layer" => continue,
                        "import" => {
                            if seen_non_import {
                                diagnostics.push(
                                    Diagnostic::new(
                                        self.name(),
                                        "Unexpected @import after other statements",
                                    )
                                    .severity(self.default_severity())
                                    .span(Span::new(
                                        at_rule.span.offset,
                                        at_rule.span.length,
                                    )),
                                );
                            }
                        }
                        _ => {
                            seen_non_import = true;
                        }
                    }
                }

                // Any other node (style rules, declarations) counts as non-import.
                _ => {
                    seen_non_import = true;
                }
            }
        }

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{AtRule, Span as ParserSpan, StyleRule, Syntax};

    #[test]
    fn reports_import_after_style_rule() {
        let rule = NoInvalidPositionAtImportRule;
        let nodes = vec![
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(0, 5),
            }),
            CssNode::AtRule(AtRule {
                name: "import".to_string(),
                params: "url.css".to_string(),
                span: ParserSpan::new(10, 20),
                children: vec![],
            }),
        ];
        let context = RuleContext {
            file_path: "test.css",
            source: "",
            syntax: Syntax::Css,
        };
        let diags = rule.check_root(&nodes, &context);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("@import"));
    }

    #[test]
    fn allows_import_before_rules() {
        let rule = NoInvalidPositionAtImportRule;
        let nodes = vec![
            CssNode::AtRule(AtRule {
                name: "import".to_string(),
                params: "reset.css".to_string(),
                span: ParserSpan::new(0, 20),
                children: vec![],
            }),
            CssNode::Style(StyleRule {
                selector: "a".to_string(),
                declarations: vec![],
                children: vec![],
                span: ParserSpan::new(25, 5),
            }),
        ];
        let context = RuleContext {
            file_path: "test.css",
            source: "",
            syntax: Syntax::Css,
        };
        let diags = rule.check_root(&nodes, &context);
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_charset_and_layer_before_import() {
        let rule = NoInvalidPositionAtImportRule;
        let nodes = vec![
            CssNode::AtRule(AtRule {
                name: "charset".to_string(),
                params: "UTF-8".to_string(),
                span: ParserSpan::new(0, 15),
                children: vec![],
            }),
            CssNode::AtRule(AtRule {
                name: "layer".to_string(),
                params: "base".to_string(),
                span: ParserSpan::new(16, 12),
                children: vec![],
            }),
            CssNode::AtRule(AtRule {
                name: "import".to_string(),
                params: "reset.css".to_string(),
                span: ParserSpan::new(30, 20),
                children: vec![],
            }),
        ];
        let context = RuleContext {
            file_path: "test.css",
            source: "",
            syntax: Syntax::Css,
        };
        let diags = rule.check_root(&nodes, &context);
        assert!(diags.is_empty());
    }
}
