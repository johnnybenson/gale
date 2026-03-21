use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Require or disallow quotes for font family names.
///
/// When the option is `"always-where-recommended"` (the default), font
/// family names that contain whitespace, digits at the start, or special
/// punctuation must be quoted. Generic family keywords (`serif`,
/// `sans-serif`, `monospace`, etc.) must NOT be quoted.
///
/// Equivalent to Stylelint's `font-family-name-quotes` rule.
pub struct FontFamilyNameQuotes;

/// Generic font families that should never be quoted.
const GENERIC_FAMILIES: &[&str] = &[
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
    "ui-serif",
    "ui-sans-serif",
    "ui-monospace",
    "ui-rounded",
    "emoji",
    "math",
    "fangsong",
    "inherit",
    "initial",
    "unset",
    "revert",
    "revert-layer",
];

fn is_generic_family(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    GENERIC_FAMILIES.iter().any(|g| *g == lower)
}

/// Returns true if a font family name needs quoting per CSS spec
/// recommendations: names containing whitespace, starting with a digit,
/// or containing punctuation other than `-`.
fn needs_quoting(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Contains whitespace → needs quoting.
    if trimmed.contains(|c: char| c.is_ascii_whitespace()) {
        return true;
    }

    // Starts with a digit → needs quoting.
    if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
        return true;
    }

    // Contains special characters (anything other than alphanumeric, `-`, `_`)
    // → needs quoting.
    if trimmed.contains(|c: char| !c.is_alphanumeric() && c != '-' && c != '_') {
        return true;
    }

    false
}

/// Parse font-family value into individual family name tokens.
/// Handles quoted and unquoted names separated by commas.
fn parse_font_families(value: &str) -> Vec<FontFamilyToken> {
    let mut families = Vec::new();
    let mut i = 0;
    let bytes = value.as_bytes();
    let len = bytes.len();

    while i < len {
        // Skip whitespace and commas.
        while i < len && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        if i >= len {
            break;
        }

        if bytes[i] == b'"' || bytes[i] == b'\'' {
            // Quoted name.
            let quote = bytes[i];
            let start = i;
            i += 1;
            let name_start = i;
            while i < len {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    break;
                }
                i += 1;
            }
            let name = &value[name_start..i];
            if i < len {
                i += 1; // skip closing quote
            }
            families.push(FontFamilyToken {
                name: name.to_string(),
                quoted: true,
                offset: start,
                length: i - start,
            });
        } else {
            // Unquoted name — may span multiple words until comma or end.
            let start = i;
            while i < len && bytes[i] != b',' {
                i += 1;
            }
            let raw = value[start..i].trim();
            if !raw.is_empty() {
                families.push(FontFamilyToken {
                    name: raw.to_string(),
                    quoted: false,
                    offset: start,
                    length: raw.len(),
                });
            }
        }
    }

    families
}

struct FontFamilyToken {
    name: String,
    quoted: bool,
    offset: usize,
    length: usize,
}

impl Rule for FontFamilyNameQuotes {
    fn name(&self) -> &'static str {
        "font-family-name-quotes"
    }

    fn description(&self) -> &'static str {
        "Require or disallow quotes for font family names"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _context: &RuleContext) -> Vec<Diagnostic> {
        let style = match node {
            CssNode::Style(s) => s,
            _ => return vec![],
        };

        let mut diagnostics = Vec::new();

        for decl in &style.declarations {
            let prop_lower = decl.property.to_ascii_lowercase();
            if prop_lower != "font-family" && prop_lower != "font" {
                continue;
            }

            let value = if prop_lower == "font" {
                // For `font` shorthand, the font-family part comes after
                // the font-size (and optional line-height). We look for
                // the last segment that starts after a size-like token.
                // Simplified: find the portion after the last `/` or after
                // what looks like a size value. For robustness, just scan
                // for family names in the whole value.
                &decl.value
            } else {
                &decl.value
            };

            let families = parse_font_families(value);

            for family in &families {
                // Skip SCSS variables and CSS custom properties.
                if family.name.starts_with('$') || family.name.starts_with("var(") {
                    continue;
                }

                if is_generic_family(&family.name) {
                    // Generic families must NOT be quoted.
                    if family.quoted {
                        diagnostics.push(
                            Diagnostic::new(
                                self.name(),
                                format!(
                                    "Unexpected quotes around generic font family \"{}\"",
                                    family.name
                                ),
                            )
                            .severity(self.default_severity())
                            .span(Span::new(decl.span.offset + family.offset, family.length)),
                        );
                    }
                } else if needs_quoting(&family.name) && !family.quoted {
                    // Non-generic families that need quoting must be quoted.
                    diagnostics.push(
                        Diagnostic::new(
                            self.name(),
                            format!(
                                "Expected quotes around font family name \"{}\"",
                                family.name
                            ),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(decl.span.offset + family.offset, family.length)),
                    );
                }
            }
        }

        diagnostics
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
            syntax: Syntax::Css,
            options: None,
        }
    }

    fn make_node(property: &str, value: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: property.to_string(),
                value: value.to_string(),
                span: ParserSpan::new(4, value.len() + property.len() + 2),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, value.len() + property.len() + 20),
        })
    }

    #[test]
    fn flags_unquoted_family_with_space() {
        let rule = FontFamilyNameQuotes;
        let node = make_node("font-family", "Times New Roman, serif");
        let diags = rule.check(&node, &make_context());
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Times New Roman"));
    }

    #[test]
    fn allows_quoted_family_with_space() {
        let rule = FontFamilyNameQuotes;
        let node = make_node("font-family", "\"Times New Roman\", serif");
        let diags = rule.check(&node, &make_context());
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_unquoted_single_word() {
        let rule = FontFamilyNameQuotes;
        let node = make_node("font-family", "Arial, sans-serif");
        let diags = rule.check(&node, &make_context());
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_quoted_generic_family() {
        let rule = FontFamilyNameQuotes;
        let node = make_node("font-family", "\"serif\"");
        let diags = rule.check(&node, &make_context());
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("generic"));
    }

    #[test]
    fn allows_unquoted_generic_family() {
        let rule = FontFamilyNameQuotes;
        let node = make_node("font-family", "monospace");
        let diags = rule.check(&node, &make_context());
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_family_starting_with_digit() {
        let rule = FontFamilyNameQuotes;
        let node = make_node("font-family", "1234, sans-serif");
        let diags = rule.check(&node, &make_context());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn is_generic_family_case_insensitive() {
        assert!(is_generic_family("Sans-Serif"));
        assert!(is_generic_family("MONOSPACE"));
        assert!(!is_generic_family("Arial"));
    }
}
