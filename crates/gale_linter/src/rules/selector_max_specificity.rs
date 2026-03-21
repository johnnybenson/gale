use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Limit the specificity of selectors.
///
/// Default maximum: "0,3,3" (0 IDs, 3 classes, 3 types).
///
/// Equivalent to Stylelint's `selector-max-specificity` rule.
pub struct SelectorMaxSpecificity;

const MAX_ID: usize = 0;
const MAX_CLASS: usize = 3;
const MAX_TYPE: usize = 3;

impl Rule for SelectorMaxSpecificity {
    fn name(&self) -> &'static str {
        "selector-max-specificity"
    }

    fn description(&self) -> &'static str {
        "Limit the specificity of selectors"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };
        let (ids, classes, types) = compute_specificity(&rule.selector);
        if ids > MAX_ID || classes > MAX_CLASS || types > MAX_TYPE {
            vec![Diagnostic::new(
                self.name(),
                format!(
                    "Expected selector \"{sel}\" to have a specificity no more than \"{MAX_ID},{MAX_CLASS},{MAX_TYPE}\", but got \"{ids},{classes},{types}\"",
                    sel = rule.selector,
                ),
            )
            .severity(self.default_severity())
            .span(Span::new(rule.span.offset, rule.span.length))]
        } else {
            vec![]
        }
    }
}

/// Compute a rough (id, class, type) specificity tuple from a selector string.
fn compute_specificity(selector: &str) -> (usize, usize, usize) {
    let mut ids = 0;
    let mut classes = 0;
    let mut types = 0;

    // Take the highest-specificity compound selector if there are commas
    let parts: Vec<&str> = selector.split(',').collect();
    for part in parts {
        let (i, c, t) = compute_single_specificity(part.trim());
        ids = ids.max(i);
        classes = classes.max(c);
        types = types.max(t);
    }

    (ids, classes, types)
}

fn compute_single_specificity(selector: &str) -> (usize, usize, usize) {
    let mut ids = 0;
    let mut classes = 0;
    let mut types = 0;
    let bytes = selector.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        match bytes[i] {
            b'#' => {
                ids += 1;
                i += 1;
                // Skip identifier
                while i < len && is_ident_char(bytes[i]) {
                    i += 1;
                }
            }
            b'.' => {
                classes += 1;
                i += 1;
                while i < len && is_ident_char(bytes[i]) {
                    i += 1;
                }
            }
            b'[' => {
                // Attribute selector counts as class-level
                classes += 1;
                i += 1;
                while i < len && bytes[i] != b']' {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
            }
            b':' => {
                i += 1;
                if i < len && bytes[i] == b':' {
                    // Pseudo-element: counts as type
                    types += 1;
                    i += 1;
                } else {
                    // Pseudo-class: counts as class-level
                    classes += 1;
                }
                while i < len && is_ident_char(bytes[i]) {
                    i += 1;
                }
                // Skip parenthetical content like :nth-child(...)
                if i < len && bytes[i] == b'(' {
                    let mut depth = 1;
                    i += 1;
                    while i < len && depth > 0 {
                        if bytes[i] == b'(' {
                            depth += 1;
                        } else if bytes[i] == b')' {
                            depth -= 1;
                        }
                        i += 1;
                    }
                }
            }
            b' ' | b'>' | b'+' | b'~' | b'*' => {
                i += 1;
            }
            _ if is_ident_start(bytes[i]) => {
                types += 1;
                while i < len && is_ident_char(bytes[i]) {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    (ids, classes, types)
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'-' || b > 127
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b > 127
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Span as ParserSpan, StyleRule, Syntax};

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
            declarations: vec![],
            children: vec![],
            span: ParserSpan::new(0, sel.len()),
        })
    }

    #[test]
    fn reports_high_specificity() {
        // #id has specificity 1,0,0 which exceeds 0,3,3
        let d = SelectorMaxSpecificity.check(&style_with_selector("#id"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("1,0,0"));
    }

    #[test]
    fn allows_low_specificity() {
        let d = SelectorMaxSpecificity.check(&style_with_selector(".a .b .c"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn reports_too_many_classes() {
        // .a.b.c.d has specificity 0,4,0 which exceeds 0,3,3
        let d = SelectorMaxSpecificity.check(&style_with_selector(".a.b.c.d"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("0,4,0"));
    }
}
