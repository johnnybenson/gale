use gale_css_parser::{CssNode, Syntax};
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Disallow deprecated global SCSS function calls that should use modules.
///
/// e.g. `adjust-color()` should be `color.adjust()`.
pub struct ScssNoGlobalFunctionNames;

/// Deprecated global SCSS function names (sorted for binary search).
static DEPRECATED_GLOBAL_FUNCTIONS: &[&str] = &[
    "abs",
    "adjust-color",
    "alpha",
    "append",
    "blue",
    "call",
    "ceil",
    "change-color",
    "comparable",
    "complement",
    "darken",
    "desaturate",
    "feature-exists",
    "floor",
    "function-exists",
    "global-variable-exists",
    "grayscale",
    "green",
    "hue",
    "ie-hex-str",
    "index",
    "invert",
    "is-superselector",
    "join",
    "keywords",
    "length",
    "lighten",
    "lightness",
    "list-separator",
    "map-get",
    "map-has-key",
    "map-keys",
    "map-merge",
    "map-remove",
    "map-values",
    "max",
    "min",
    "mix",
    "mixin-exists",
    "nth",
    "opacify",
    "percentage",
    "quote",
    "random",
    "red",
    "round",
    "saturate",
    "saturation",
    "scale-color",
    "selector-append",
    "selector-extend",
    "selector-nest",
    "selector-replace",
    "selector-unify",
    "simple-selectors",
    "str-index",
    "str-insert",
    "str-length",
    "str-slice",
    "to-lower-case",
    "to-upper-case",
    "transparentize",
    "type-of",
    "unique-id",
    "unit",
    "unitless",
    "unquote",
    "variable-exists",
    "zip",
];

/// Functions that are also valid CSS natives and should NOT be flagged.
/// These are Sass functions that overlap with CSS math/color/filter functions.
static CSS_NATIVE_FUNCTIONS: &[&str] = &[
    "abs",
    "alpha",
    "blue",
    "ceil",
    "floor",
    "grayscale",
    "green",
    "hue",
    "invert",
    "max",
    "min",
    "percentage",
    "red",
    "round",
    "saturate",
];

fn is_deprecated_global(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    DEPRECATED_GLOBAL_FUNCTIONS
        .binary_search(&lower.as_str())
        .is_ok()
}

fn is_css_native(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    CSS_NATIVE_FUNCTIONS.binary_search(&lower.as_str()).is_ok()
}

impl Rule for ScssNoGlobalFunctionNames {
    fn name(&self) -> &'static str {
        "scss/no-global-function-names"
    }

    fn description(&self) -> &'static str {
        "Disallow global function names that should use Sass modules"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, ctx: &RuleContext) -> Vec<Diagnostic> {
        if !matches!(ctx.syntax, Syntax::Scss | Syntax::Sass) {
            return vec![];
        }

        let CssNode::Declaration(decl) = node else {
            return vec![];
        };

        let mut diagnostics = Vec::new();
        // Scan the declaration value for function calls: `name(`
        // Simple approach: find patterns like `word-chars(`
        let value = &decl.value;
        let bytes = value.as_bytes();
        let len = bytes.len();
        let mut i = 0;
        let mut in_single_quote = false;
        let mut in_double_quote = false;

        while i < len {
            // Track string context to skip function names inside strings
            if bytes[i] == b'"' && !in_single_quote {
                in_double_quote = !in_double_quote;
                i += 1;
                continue;
            }
            if bytes[i] == b'\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
                i += 1;
                continue;
            }
            if in_single_quote || in_double_quote {
                i += 1;
                continue;
            }

            // Skip non-alpha/hyphen
            if !bytes[i].is_ascii_alphabetic() && bytes[i] != b'-' {
                i += 1;
                continue;
            }

            // Collect function name
            let start = i;
            while i < len
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-' || bytes[i] == b'_')
            {
                i += 1;
            }

            // Check if followed by `(`
            if i < len && bytes[i] == b'(' {
                let func_name = &value[start..i];

                // Skip namespaced calls (e.g., `math.div(`, `map.get(`)
                let is_namespaced = start > 0 && bytes[start - 1] == b'.';

                if !is_namespaced && is_deprecated_global(func_name) && !is_css_native(func_name) {
                    diagnostics.push(
                        Diagnostic::new(
                            self.name(),
                            format!(
                                "Expected use of a Sass module instead of global function \"{}\"",
                                func_name
                            ),
                        )
                        .severity(self.default_severity())
                        .span(Span::new(decl.span.offset, decl.span.length)),
                    );
                }
            }

            i += 1;
        }

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Declaration, Span as ParserSpan, Syntax};

    fn scss_ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "t.scss",
            source: "",
            syntax: Syntax::Scss,
            options: None,
        }
    }

    fn css_ctx() -> RuleContext<'static> {
        RuleContext {
            file_path: "t.css",
            source: "",
            syntax: Syntax::Css,
            options: None,
        }
    }

    fn decl(value: &str) -> CssNode {
        CssNode::Declaration(Declaration {
            property: "color".to_string(),
            value: value.to_string(),
            span: ParserSpan::new(0, 10),
            important: false,
        })
    }

    #[test]
    fn skips_non_scss() {
        assert!(
            ScssNoGlobalFunctionNames
                .check(&decl("adjust-color(red, $red: 10)"), &css_ctx())
                .is_empty()
        );
    }

    #[test]
    fn reports_deprecated_global() {
        let d = ScssNoGlobalFunctionNames.check(&decl("adjust-color(red, $red: 10)"), &scss_ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("adjust-color"));
    }

    #[test]
    fn reports_map_get() {
        let d = ScssNoGlobalFunctionNames.check(&decl("map-get($map, key)"), &scss_ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("map-get"));
    }

    #[test]
    fn allows_module_function() {
        // `color.adjust()` is not a deprecated global.
        let d = ScssNoGlobalFunctionNames.check(&decl("color.adjust(red, $red: 10)"), &scss_ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_namespaced_map_get() {
        // `map.get()` should not be flagged — it uses a namespace
        let d = ScssNoGlobalFunctionNames.check(&decl("map.get($map, key)"), &scss_ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_css_native_min_max() {
        let d = ScssNoGlobalFunctionNames.check(&decl("min(100px, 50vw)"), &scss_ctx());
        assert!(d.is_empty());
        let d = ScssNoGlobalFunctionNames.check(&decl("max(100px, 50vw)"), &scss_ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_css_native_round() {
        let d = ScssNoGlobalFunctionNames.check(&decl("round(1.5)"), &scss_ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_function_in_string() {
        let d = ScssNoGlobalFunctionNames.check(&decl("\"use map-get() instead\""), &scss_ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn allows_non_deprecated() {
        let d = ScssNoGlobalFunctionNames.check(&decl("rgba(0, 0, 0, 0.5)"), &scss_ctx());
        assert!(d.is_empty());
    }

    #[test]
    fn sorted_array() {
        for window in DEPRECATED_GLOBAL_FUNCTIONS.windows(2) {
            assert!(
                window[0] < window[1],
                "'{}' should come before '{}'",
                window[0],
                window[1]
            );
        }
    }
}
