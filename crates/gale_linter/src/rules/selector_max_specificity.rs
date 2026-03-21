use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Limit the specificity of selectors.
///
/// Accepts a specificity string like `"0,2,0"` as the primary option,
/// representing max (id, class, type). Defaults to `"0,2,0"` if no option
/// is provided.
///
/// Equivalent to Stylelint's `selector-max-specificity` rule.
pub struct SelectorMaxSpecificity;

const DEFAULT_MAX_ID: usize = 0;
const DEFAULT_MAX_CLASS: usize = 2;
const DEFAULT_MAX_TYPE: usize = 0;

/// Parse a specificity string like `"0,2,0"` into `(id, class, type)`.
fn parse_specificity_option(s: &str) -> Option<(usize, usize, usize)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        return None;
    }
    let id = parts[0].trim().parse::<usize>().ok()?;
    let class = parts[1].trim().parse::<usize>().ok()?;
    let typ = parts[2].trim().parse::<usize>().ok()?;
    Some((id, class, typ))
}

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

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        // Read the max specificity from options, or use the default "0,2,0".
        let (max_id, max_class, max_type) = ctx
            .options
            .and_then(|v| v.as_str())
            .and_then(parse_specificity_option)
            .unwrap_or((DEFAULT_MAX_ID, DEFAULT_MAX_CLASS, DEFAULT_MAX_TYPE));

        let (ids, classes, types) = compute_specificity(&rule.selector);
        if ids > max_id || classes > max_class || types > max_type {
            vec![Diagnostic::new(
                self.name(),
                format!(
                    "Expected selector \"{sel}\" to have a specificity no more than \"{max_id},{max_class},{max_type}\", but got \"{ids},{classes},{types}\"",
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

/// Extract the content inside balanced parentheses from a byte string,
/// advancing `i` past the closing `)`.  Assumes `selector.as_bytes()[*i] == b'('`.
fn extract_paren_content<'a>(selector: &'a str, i: &mut usize) -> &'a str {
    let bytes = selector.as_bytes();
    let mut depth = 1;
    *i += 1; // skip opening '('
    let start = *i;
    while *i < bytes.len() && depth > 0 {
        if bytes[*i] == b'(' {
            depth += 1;
        } else if bytes[*i] == b')' {
            depth -= 1;
        }
        if depth > 0 {
            *i += 1;
        }
    }
    let end = *i;
    if *i < bytes.len() {
        *i += 1; // skip closing ')'
    }
    &selector[start..end]
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
                    while i < len && is_ident_char(bytes[i]) {
                        i += 1;
                    }
                    // Skip parenthetical content
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
                } else {
                    // Pseudo-class -- read the name first
                    let name_start = i;
                    while i < len && is_ident_char(bytes[i]) {
                        i += 1;
                    }
                    let name = &selector[name_start..i];

                    if name.eq_ignore_ascii_case("where") {
                        // :where() has zero specificity -- skip arguments
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
                    } else if name.eq_ignore_ascii_case("is")
                        || name.eq_ignore_ascii_case("not")
                        || name.eq_ignore_ascii_case("has")
                    {
                        // :is(), :not(), :has() take the specificity of the
                        // most specific argument.
                        if i < len && bytes[i] == b'(' {
                            let inner = extract_paren_content(selector, &mut i);
                            let mut max_i = 0;
                            let mut max_c = 0;
                            let mut max_t = 0;
                            for arg in inner.split(',') {
                                let (ai, ac, at) = compute_single_specificity(arg.trim());
                                max_i = max_i.max(ai);
                                max_c = max_c.max(ac);
                                max_t = max_t.max(at);
                            }
                            ids += max_i;
                            classes += max_c;
                            types += max_t;
                        }
                    } else {
                        classes += 1;
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

    fn ctx_with_options(opts: &serde_json::Value) -> RuleContext<'_> {
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: Some(opts),
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
        // #id has specificity 1,0,0 which exceeds default 0,2,0
        let d = SelectorMaxSpecificity.check(&style_with_selector("#id"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("1,0,0"));
    }

    #[test]
    fn allows_low_specificity() {
        // .a .b has specificity 0,2,0 which is within default 0,2,0
        let d = SelectorMaxSpecificity.check(&style_with_selector(".a .b"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn reports_too_many_classes() {
        // .a.b.c has specificity 0,3,0 which exceeds default 0,2,0
        let d = SelectorMaxSpecificity.check(&style_with_selector(".a.b.c"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("0,3,0"));
    }

    #[test]
    fn where_has_zero_specificity() {
        // :where(#id) should have (0,0,0), well under the limit
        let d = SelectorMaxSpecificity.check(&style_with_selector(":where(#id)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn is_takes_max_argument_specificity() {
        // :is(#id) should have (1,0,0) which exceeds limit
        let d = SelectorMaxSpecificity.check(&style_with_selector(":is(#id)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("1,0,0"));

        // :is(.a) should have (0,1,0) which is within default 0,2,0
        let d = SelectorMaxSpecificity.check(&style_with_selector(":is(.a)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn not_takes_max_argument_specificity() {
        // :not(#id) should have (1,0,0) which exceeds limit
        let d = SelectorMaxSpecificity.check(&style_with_selector(":not(#id)"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("1,0,0"));
    }

    #[test]
    fn custom_option_allows_higher_specificity() {
        let opts = serde_json::json!("0,4,1");
        let c = ctx_with_options(&opts);
        // .a.b.c.d has specificity 0,4,0 -- within "0,4,1"
        let d = SelectorMaxSpecificity.check(&style_with_selector(".a.b.c.d"), &c);
        assert!(d.is_empty());
    }

    #[test]
    fn custom_option_restricts_specificity() {
        let opts = serde_json::json!("0,1,0");
        let c = ctx_with_options(&opts);
        // .a.b has specificity 0,2,0 -- exceeds "0,1,0"
        let d = SelectorMaxSpecificity.check(&style_with_selector(".a.b"), &c);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("0,1,0"));
    }

    #[test]
    fn parse_specificity_option_valid() {
        assert_eq!(parse_specificity_option("0,2,0"), Some((0, 2, 0)));
        assert_eq!(parse_specificity_option("1, 3, 2"), Some((1, 3, 2)));
    }

    #[test]
    fn parse_specificity_option_invalid() {
        assert_eq!(parse_specificity_option("abc"), None);
        assert_eq!(parse_specificity_option("0,2"), None);
        assert_eq!(parse_specificity_option("0,2,0,1"), None);
    }
}
