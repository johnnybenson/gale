use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow/require named colors in CSS declarations.
///
/// Equivalent to Stylelint's `color-named` rule.
/// Options: `"never"` (default) disallows named colors; `"always-where-possible"` requires them.
pub struct ColorNamed;

/// Full list of CSS named colors (Level 4).
const NAMED_COLORS: &[&str] = &[
    "aliceblue",
    "antiquewhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedalmond",
    "blue",
    "blueviolet",
    "brown",
    "burlywood",
    "cadetblue",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerblue",
    "cornsilk",
    "crimson",
    "cyan",
    "darkblue",
    "darkcyan",
    "darkgoldenrod",
    "darkgray",
    "darkgreen",
    "darkgrey",
    "darkkhaki",
    "darkmagenta",
    "darkolivegreen",
    "darkorange",
    "darkorchid",
    "darkred",
    "darksalmon",
    "darkseagreen",
    "darkslateblue",
    "darkslategray",
    "darkslategrey",
    "darkturquoise",
    "darkviolet",
    "deeppink",
    "deepskyblue",
    "dimgray",
    "dimgrey",
    "dodgerblue",
    "firebrick",
    "floralwhite",
    "forestgreen",
    "fuchsia",
    "gainsboro",
    "ghostwhite",
    "gold",
    "goldenrod",
    "gray",
    "green",
    "greenyellow",
    "grey",
    "honeydew",
    "hotpink",
    "indianred",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderblush",
    "lawngreen",
    "lemonchiffon",
    "lightblue",
    "lightcoral",
    "lightcyan",
    "lightgoldenrodyellow",
    "lightgray",
    "lightgreen",
    "lightgrey",
    "lightpink",
    "lightsalmon",
    "lightseagreen",
    "lightskyblue",
    "lightslategray",
    "lightslategrey",
    "lightsteelblue",
    "lightyellow",
    "lime",
    "limegreen",
    "linen",
    "magenta",
    "maroon",
    "mediumaquamarine",
    "mediumblue",
    "mediumorchid",
    "mediumpurple",
    "mediumseagreen",
    "mediumslateblue",
    "mediumspringgreen",
    "mediumturquoise",
    "mediumvioletred",
    "midnightblue",
    "mintcream",
    "mistyrose",
    "moccasin",
    "navajowhite",
    "navy",
    "oldlace",
    "olive",
    "olivedrab",
    "orange",
    "orangered",
    "orchid",
    "palegoldenrod",
    "palegreen",
    "paleturquoise",
    "palevioletred",
    "papayawhip",
    "peachpuff",
    "peru",
    "pink",
    "plum",
    "powderblue",
    "purple",
    "rebeccapurple",
    "red",
    "rosybrown",
    "royalblue",
    "saddlebrown",
    "salmon",
    "sandybrown",
    "seagreen",
    "seashell",
    "sienna",
    "silver",
    "skyblue",
    "slateblue",
    "slategray",
    "slategrey",
    "snow",
    "springgreen",
    "steelblue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "turquoise",
    "violet",
    "wheat",
    "white",
    "whitesmoke",
    "yellow",
    "yellowgreen",
];

impl Rule for ColorNamed {
    fn name(&self) -> &'static str {
        "color-named"
    }

    fn description(&self) -> &'static str {
        "Require or disallow named colors"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::Style(rule) = node else {
            return vec![];
        };

        // Read the primary option: "never" (default) or "always-where-possible"
        // Options may be a plain string or array form ["never", { secondary }].
        let option = ctx.primary_option_str().unwrap_or("never");

        // Only "never" mode is implemented (disallow named colors)
        if option != "never" {
            return vec![];
        }

        let mut diags = Vec::new();
        for decl in &rule.declarations {
            // Use the original source text when available, because lightningcss
            // may serialise hex colours to named-colour equivalents (e.g.
            // `#f00` → `red`), which would cause false positives.
            let decl_start = decl.span.offset;
            let decl_end = decl_start + decl.span.length;
            let value = if decl_end <= ctx.source.len() && decl_start < decl_end {
                // Extract just the value portion from "property: value"
                let full = &ctx.source[decl_start..decl_end];
                // Find the colon that separates property from value
                if let Some(colon_pos) = full.find(':') {
                    full[colon_pos + 1..].trim_end_matches(';').trim()
                } else {
                    full
                }
            } else {
                &decl.value
            };

            // Strip out content inside var(), #{} interpolation, and SCSS $variables
            let cleaned = strip_ignored_parts(value);
            let cleaned_lower = cleaned.to_ascii_lowercase();

            for &color in NAMED_COLORS {
                if contains_color_word(&cleaned_lower, color) {
                    // Try to locate the named color within the source text for
                    // an accurate span, falling back to the declaration span.
                    let color_span = if decl_end <= ctx.source.len() && decl_start < decl_end {
                        let full = &ctx.source[decl_start..decl_end];
                        find_color_word_offset(&full.to_ascii_lowercase(), color)
                            .map(|rel| Span::new(decl_start + rel, color.len()))
                            .unwrap_or_else(|| Span::new(decl.span.offset, decl.span.length))
                    } else {
                        Span::new(decl.span.offset, decl.span.length)
                    };
                    diags.push(
                        Diagnostic::new(self.name(), format!("Unexpected named color \"{color}\""))
                            .severity(self.default_severity())
                            .span(color_span),
                    );
                }
            }
        }
        diags
    }
}

/// Strip parts of a value that should not be checked:
/// - Quoted strings (`"..."` and `'...'`) — e.g. `govuk-colour("white")`
/// - `var(...)` function calls (including nested parens)
/// - `#{...}` SCSS interpolation
/// - `$variable-name` SCSS variables
fn strip_ignored_parts(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Skip quoted strings (single or double)
        if chars[i] == '"' || chars[i] == '\'' {
            let quote = chars[i];
            i += 1;
            while i < len && chars[i] != quote {
                if chars[i] == '\\' {
                    i += 1; // skip escaped char
                }
                i += 1;
            }
            if i < len {
                i += 1; // skip closing quote
            }
            result.push(' ');
            continue;
        }

        // Skip SCSS interpolation #{...}
        if i + 1 < len && chars[i] == '#' && chars[i + 1] == '{' {
            let mut depth = 1;
            i += 2;
            while i < len && depth > 0 {
                if chars[i] == '{' {
                    depth += 1;
                } else if chars[i] == '}' {
                    depth -= 1;
                }
                i += 1;
            }
            result.push(' ');
            continue;
        }

        // Skip SCSS variables $name
        if chars[i] == '$' {
            i += 1;
            while i < len
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '-' || chars[i] == '_')
            {
                i += 1;
            }
            result.push(' ');
            continue;
        }

        // Skip var(...) with nested parens
        if i + 3 < len
            && chars[i].eq_ignore_ascii_case(&'v')
            && chars[i + 1].eq_ignore_ascii_case(&'a')
            && chars[i + 2].eq_ignore_ascii_case(&'r')
            && chars[i + 3] == '('
        {
            // Make sure 'var' is a word boundary (not part of a longer identifier)
            let before_ok = i == 0 || !chars[i - 1].is_ascii_alphanumeric();
            if before_ok {
                let mut depth = 1;
                i += 4; // skip "var("
                while i < len && depth > 0 {
                    if chars[i] == '(' {
                        depth += 1;
                    } else if chars[i] == ')' {
                        depth -= 1;
                    }
                    i += 1;
                }
                result.push(' ');
                continue;
            }
        }

        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Find the byte offset of a named color `word` as a whole word within `haystack`.
/// Returns `None` if not found as a whole word.
fn find_color_word_offset(haystack: &str, word: &str) -> Option<usize> {
    let bytes = haystack.as_bytes();
    let word_bytes = word.as_bytes();
    let wlen = word_bytes.len();
    if bytes.len() < wlen {
        return None;
    }
    for i in 0..=(bytes.len() - wlen) {
        if &bytes[i..i + wlen] == word_bytes {
            let before_ok = i == 0
                || !(bytes[i - 1].is_ascii_alphanumeric()
                    || bytes[i - 1] == b'-'
                    || bytes[i - 1] == b'_');
            let after_ok = i + wlen == bytes.len()
                || !(bytes[i + wlen].is_ascii_alphanumeric()
                    || bytes[i + wlen] == b'-'
                    || bytes[i + wlen] == b'_');
            if before_ok && after_ok {
                return Some(i);
            }
        }
    }
    None
}

/// Check if `haystack` contains `word` as a whole word (bounded by non-alphanumeric, non-hyphen chars).
/// We exclude hyphen from word boundary so that e.g. "dark-red" doesn't false-positive on "red",
/// but "border-color: red" does match (space before, end-of-string after).
fn contains_color_word(haystack: &str, word: &str) -> bool {
    let bytes = haystack.as_bytes();
    let word_bytes = word.as_bytes();
    let wlen = word_bytes.len();
    if bytes.len() < wlen {
        return false;
    }
    for i in 0..=(bytes.len() - wlen) {
        if &bytes[i..i + wlen] == word_bytes {
            // Before: must be start of string or non-ident char
            let before_ok = i == 0
                || !(bytes[i - 1].is_ascii_alphanumeric()
                    || bytes[i - 1] == b'-'
                    || bytes[i - 1] == b'_');
            // After: must be end of string or non-ident char
            let after_ok = i + wlen == bytes.len()
                || !(bytes[i + wlen].is_ascii_alphanumeric()
                    || bytes[i + wlen] == b'-'
                    || bytes[i + wlen] == b'_');
            if before_ok && after_ok {
                return true;
            }
        }
    }
    false
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

    fn style_with_value(value: &str) -> CssNode {
        CssNode::Style(StyleRule {
            selector: "a".to_string(),
            declarations: vec![Declaration {
                property: "color".to_string(),
                value: value.to_string(),
                span: ParserSpan::new(0, 0),
                important: false,
            }],
            children: vec![],
            span: ParserSpan::new(0, 0),
        })
    }

    #[test]
    fn reports_named_color() {
        let d = ColorNamed.check(&style_with_value("red"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("red"));
    }

    #[test]
    fn reports_named_color_case_insensitive() {
        let d = ColorNamed.check(&style_with_value("Red"), &ctx());
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn allows_hex_color() {
        let d = ColorNamed.check(&style_with_value("#ff0000"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn does_not_match_partial_words() {
        // "grayed" should not match "gray"
        let d = ColorNamed.check(&style_with_value("grayed"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn does_not_match_inside_compound_name() {
        // "darkred" is its own named color, but "red" alone should not match inside it
        let d = ColorNamed.check(&style_with_value("darkred"), &ctx());
        // Should report "darkred" but NOT "red" separately
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("darkred"));
    }

    #[test]
    fn skips_var_function() {
        let d = ColorNamed.check(&style_with_value("var(--red)"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn skips_scss_variable() {
        let d = ColorNamed.check(&style_with_value("$red-color"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn skips_scss_interpolation() {
        let d = ColorNamed.check(&style_with_value("#{$red}"), &ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn reports_color_outside_var() {
        let d = ColorNamed.check(&style_with_value("var(--bg) red"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("red"));
    }
}
