use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use globset::{Glob, GlobMatcher};
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("failed to parse YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("failed to parse TOML: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("unsupported config file format: {0}")]
    UnsupportedFormat(String),
}

// ---------------------------------------------------------------------------
// Public enums
// ---------------------------------------------------------------------------

/// Rule severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Off,
}

/// Output formatter type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FormatterType {
    #[default]
    Text,
    Json,
    Compact,
}

// ---------------------------------------------------------------------------
// Resolved config (public API)
// ---------------------------------------------------------------------------

/// A single rule's resolved configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleConfig {
    pub severity: Option<Severity>,
    pub options: Option<serde_json::Value>,
}

/// A resolved override entry: a set of glob patterns and the rules to apply
/// when a file matches any of them.
#[derive(Debug, Clone)]
pub struct ResolvedOverride {
    pub file_patterns: Vec<String>,
    matchers: Vec<GlobMatcher>,
    pub rules: HashMap<String, RuleConfig>,
}

impl ResolvedOverride {
    /// Create a new resolved override from glob pattern strings and rules.
    pub fn new(file_patterns: Vec<String>, rules: HashMap<String, RuleConfig>) -> Self {
        let matchers = file_patterns
            .iter()
            .filter_map(|pat| Glob::new(pat).ok().map(|g| g.compile_matcher()))
            .collect();
        Self {
            file_patterns,
            matchers,
            rules,
        }
    }

    /// Check whether a file path matches any of this override's glob patterns.
    pub fn matches(&self, file_path: &str) -> bool {
        let path = Path::new(file_path);
        self.matchers.iter().any(|m| m.is_match(path))
    }
}

/// The fully-resolved configuration used by the linter at runtime.
#[derive(Debug, Clone)]
pub struct GaleConfig {
    pub rules: HashMap<String, RuleConfig>,
    pub ignore_patterns: Vec<String>,
    pub formatter: FormatterType,
    pub overrides: Vec<ResolvedOverride>,
}

impl GaleConfig {
    /// Return the effective rules for a given file path.
    ///
    /// Starts with the base rules, then applies each matching override in order.
    /// Later overrides win over earlier ones.
    pub fn rules_for_file(&self, file_path: &str) -> HashMap<String, RuleConfig> {
        if self.overrides.is_empty() {
            return self.rules.clone();
        }

        let mut rules = self.rules.clone();
        for override_entry in &self.overrides {
            if override_entry.matches(file_path) {
                for (name, cfg) in &override_entry.rules {
                    if cfg.severity == Some(Severity::Off) {
                        rules.remove(name);
                    } else {
                        rules.insert(name.clone(), cfg.clone());
                    }
                }
            }
        }
        rules
    }

    /// Returns `true` if any overrides are configured.
    pub fn has_overrides(&self) -> bool {
        !self.overrides.is_empty()
    }
}

impl Default for GaleConfig {
    fn default() -> Self {
        Self {
            rules: HashMap::new(),
            ignore_patterns: Vec::new(),
            formatter: FormatterType::Text,
            overrides: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Raw config file representation (serde)
// ---------------------------------------------------------------------------

/// Deserialize a value that can be either a single string or an array of strings.
fn string_or_vec<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        Single(String),
        Multiple(Vec<String>),
    }

    Option::<StringOrVec>::deserialize(deserializer).map(|opt| {
        opt.map(|v| match v {
            StringOrVec::Single(s) => vec![s],
            StringOrVec::Multiple(v) => v,
        })
    })
}

/// What is actually stored in a config file on disk.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFile {
    pub rules: Option<HashMap<String, RuleConfigValue>>,
    pub ignore_patterns: Option<Vec<String>>,
    pub formatter: Option<String>,
    /// List of shared configs / presets to extend (e.g. `"gale:recommended"`).
    /// Accepts a single string or an array of strings.
    #[serde(default, deserialize_with = "string_or_vec")]
    pub extends: Option<Vec<String>>,
    /// File-pattern-based overrides (like Stylelint's `overrides` field).
    pub overrides: Option<Vec<ConfigOverride>>,
}

/// A single override entry as it appears in the config file.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfigOverride {
    /// Glob patterns for files this override applies to.
    /// Accepts a single string or an array of strings.
    #[serde(default, deserialize_with = "string_or_vec")]
    pub files: Option<Vec<String>>,
    /// Rules to apply for matching files.
    pub rules: Option<HashMap<String, RuleConfigValue>>,
    /// Shared configs to extend for matching files.
    #[serde(default, deserialize_with = "string_or_vec")]
    pub extends: Option<Vec<String>>,
}

/// A serde-friendly enum matching Stylelint's flexible rule value format.
///
/// Accepts any of:
/// - `null`             (null — treated as off)
/// - `true` / `false`  (boolean — true means error, false means off)
/// - `0`               (numeric zero — treated as off)
/// - `"error"` / `"warning"` / `"off"` (string severity)
/// - `["error", { ...options }]` (tuple of severity + options)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleConfigValue {
    /// Null: treated as Off (used in JS configs to disable inherited rules).
    Null(Option<()>),
    /// Boolean shorthand: `true` → Error, `false` → Off.
    Bool(bool),
    /// Numeric value (e.g. `0` for max-rules).
    Number(serde_json::Number),
    /// String severity: `"error"`, `"warning"`, `"off"`.
    Severity(String),
    /// Array form: `["error", { ...options }]`.
    Array(Vec<serde_json::Value>),
}

impl RuleConfigValue {
    /// Convert the raw config value into a resolved [`RuleConfig`].
    pub fn resolve(&self) -> RuleConfig {
        match self {
            RuleConfigValue::Null(_) => RuleConfig {
                severity: Some(Severity::Off),
                options: None,
            },
            RuleConfigValue::Bool(b) => RuleConfig {
                severity: Some(if *b { Severity::Error } else { Severity::Off }),
                options: None,
            },
            RuleConfigValue::Number(n) => {
                // Numeric value — store as the primary option.
                // A value of 0 is treated as Off (common pattern for max-* rules).
                let is_zero = n.as_u64() == Some(0);
                RuleConfig {
                    severity: Some(if is_zero {
                        Severity::Off
                    } else {
                        Severity::Error
                    }),
                    options: Some(serde_json::Value::Number(n.clone())),
                }
            }
            RuleConfigValue::Severity(s) => RuleConfig {
                severity: Some(parse_severity(s)),
                options: None,
            },
            RuleConfigValue::Array(items) => {
                let severity = items
                    .first()
                    .and_then(|v| v.as_str())
                    .map(parse_severity);
                let options = items.get(1).cloned();
                RuleConfig { severity, options }
            }
        }
    }
}

fn parse_severity(s: &str) -> Severity {
    match s.to_lowercase().as_str() {
        "error" => Severity::Error,
        "warning" | "warn" => Severity::Warning,
        _ => Severity::Off,
    }
}

// ---------------------------------------------------------------------------
// Built-in presets
// ---------------------------------------------------------------------------

/// All rule names that exist in the linter registry.
/// Kept in sync with `gale_linter::rules::register_all`.
const ALL_RULE_NAMES: &[&str] = &[
    "alpha-value-notation",
    "annotation-no-unknown",
    "at-rule-no-unknown",
    "at-rule-no-vendor-prefix",
    "block-no-empty",
    "color-hex-case",
    "color-hex-length",
    "color-no-invalid-hex",
    "comment-no-empty",
    "custom-property-no-missing-var-function",
    "declaration-block-no-duplicate-custom-properties",
    "declaration-block-no-duplicate-properties",
    "declaration-block-no-redundant-longhand-properties",
    "declaration-block-no-shorthand-property-overrides",
    "declaration-empty-line-before",
    "declaration-no-important",
    "font-family-no-duplicate-names",
    "font-family-no-missing-generic-family-keyword",
    "function-calc-no-unspaced-operator",
    "function-name-case",
    "function-url-quotes",
    "import-notation",
    "keyframe-block-no-duplicate-selectors",
    "keyframe-declaration-no-important",
    "length-zero-no-unit",
    "media-feature-name-no-unknown",
    "media-query-no-invalid",
    "no-descending-specificity",
    "no-duplicate-at-import-rules",
    "no-duplicate-selectors",
    "no-empty-source",
    "no-invalid-double-slash-comments",
    "no-invalid-position-at-import-rule",
    "no-invalid-position-declaration",
    "no-irregular-whitespace",
    "no-unknown-animations",
    "number-max-precision",
    "property-no-unknown",
    "property-no-vendor-prefix",
    "selector-class-pattern",
    "selector-pseudo-class-no-unknown",
    "selector-pseudo-element-colon-notation",
    "selector-pseudo-element-no-unknown",
    "selector-type-no-unknown",
    "shorthand-property-no-redundant-values",
    "string-no-newline",
    "unit-no-unknown",
    "value-keyword-case",
    "value-no-vendor-prefix",
];

/// Rules enabled at **error** severity in `gale:recommended`.
const RECOMMENDED_ERROR_RULES: &[&str] = &[
    "block-no-empty",
    "color-no-invalid-hex",
    "declaration-block-no-duplicate-properties",
    "declaration-block-no-duplicate-custom-properties",
    "font-family-no-duplicate-names",
    "no-duplicate-at-import-rules",
    "no-duplicate-selectors",
    "no-empty-source",
    "property-no-unknown",
    "selector-pseudo-class-no-unknown",
    "selector-pseudo-element-no-unknown",
    "selector-type-no-unknown",
    "unit-no-unknown",
    "no-descending-specificity",
    "keyframe-block-no-duplicate-selectors",
];

/// Rules enabled at **warning** severity in `gale:recommended`.
const RECOMMENDED_WARNING_RULES: &[&str] = &[
    "color-hex-length",
    "color-hex-case",
    "length-zero-no-unit",
    "declaration-no-important",
    "selector-pseudo-element-colon-notation",
    "no-invalid-double-slash-comments",
    "function-name-case",
    "shorthand-property-no-redundant-values",
    "at-rule-no-vendor-prefix",
    "property-no-vendor-prefix",
    "value-no-vendor-prefix",
    "value-keyword-case",
    "function-url-quotes",
    "number-max-precision",
];

/// Rules enabled at **warning** severity in `stylelint-config-recommended`.
///
/// These mirror the official `stylelint-config-recommended` package, filtered
/// to only include rules that Gale has implemented.
const STYLELINT_RECOMMENDED_RULES: &[&str] = &[
    "at-rule-no-unknown",
    "block-no-empty",
    "color-no-invalid-hex",
    "comment-no-empty",
    "custom-property-no-missing-var-function",
    "declaration-block-no-duplicate-custom-properties",
    "declaration-block-no-duplicate-properties",
    "declaration-block-no-shorthand-property-overrides",
    "font-family-no-duplicate-names",
    "font-family-no-missing-generic-family-keyword",
    "function-calc-no-unspaced-operator",
    "keyframe-block-no-duplicate-selectors",
    "keyframe-declaration-no-important",
    "media-feature-name-no-unknown",
    "no-descending-specificity",
    "no-duplicate-at-import-rules",
    "no-duplicate-selectors",
    "no-empty-source",
    "no-invalid-double-slash-comments",
    "no-invalid-position-at-import-rule",
    "no-irregular-whitespace",
    "property-no-unknown",
    "selector-pseudo-class-no-unknown",
    "selector-pseudo-element-no-unknown",
    "selector-type-no-unknown",
    "string-no-newline",
    "unit-no-unknown",
];

/// Additional rules that `stylelint-config-standard` adds on top of
/// `stylelint-config-recommended`.  All enabled at warning severity.
///
/// Filtered to only include rules that Gale has implemented.
const STYLELINT_STANDARD_EXTRA_RULES: &[&str] = &[
    "alpha-value-notation",
    "color-hex-length",
    "comment-empty-line-before",
    "declaration-block-no-redundant-longhand-properties",
    "declaration-empty-line-before",
    "function-name-case",
    "function-url-quotes",
    "import-notation",
    "length-zero-no-unit",
    "number-max-precision",
    "property-no-vendor-prefix",
    "rule-empty-line-before",
    "selector-class-pattern",
    "selector-pseudo-element-colon-notation",
    "shorthand-property-no-redundant-values",
    "value-keyword-case",
    "value-no-vendor-prefix",
];

/// Resolve a built-in preset name into a map of rule configurations.
///
/// Returns `None` if the preset name is not recognised.
pub fn resolve_preset(name: &str) -> Option<HashMap<String, RuleConfig>> {
    match name {
        "gale:recommended" => {
            let mut rules = HashMap::new();
            for &rule in RECOMMENDED_ERROR_RULES {
                rules.insert(
                    rule.to_string(),
                    RuleConfig {
                        severity: Some(Severity::Error),
                        options: None,
                    },
                );
            }
            for &rule in RECOMMENDED_WARNING_RULES {
                rules.insert(
                    rule.to_string(),
                    RuleConfig {
                        severity: Some(Severity::Warning),
                        options: None,
                    },
                );
            }
            Some(rules)
        }
        "gale:all" => {
            let mut rules = HashMap::new();
            for &rule in ALL_RULE_NAMES {
                rules.insert(
                    rule.to_string(),
                    RuleConfig {
                        severity: Some(Severity::Warning),
                        options: None,
                    },
                );
            }
            Some(rules)
        }
        "stylelint-config-recommended" => {
            let mut rules = HashMap::new();
            for &rule in STYLELINT_RECOMMENDED_RULES {
                rules.insert(
                    rule.to_string(),
                    RuleConfig {
                        severity: Some(Severity::Warning),
                        options: None,
                    },
                );
            }
            Some(rules)
        }
        "stylelint-config-recommended-scss" => {
            let mut rules = HashMap::new();
            for &rule in STYLELINT_RECOMMENDED_RULES {
                rules.insert(
                    rule.to_string(),
                    RuleConfig {
                        severity: Some(Severity::Warning),
                        options: None,
                    },
                );
            }
            // The real stylelint-config-recommended-scss disables these core rules
            // because SCSS has its own replacements (scss/at-rule-no-unknown, etc.)
            for &rule in &[
                "at-rule-no-unknown",
                "comment-no-empty",
                "no-duplicate-selectors",
                "no-invalid-position-at-import-rule",
            ] {
                rules.insert(
                    rule.to_string(),
                    RuleConfig {
                        severity: Some(Severity::Off),
                        options: None,
                    },
                );
            }
            Some(rules)
        }
        "stylelint-config-standard" => {
            // Standard extends recommended, then adds extra rules.
            let mut rules = HashMap::new();
            for &rule in STYLELINT_RECOMMENDED_RULES {
                rules.insert(
                    rule.to_string(),
                    RuleConfig {
                        severity: Some(Severity::Warning),
                        options: None,
                    },
                );
            }
            for &rule in STYLELINT_STANDARD_EXTRA_RULES {
                rules.insert(
                    rule.to_string(),
                    RuleConfig {
                        severity: Some(Severity::Warning),
                        options: None,
                    },
                );
            }
            Some(rules)
        }
        "stylelint-config-standard-scss" => {
            // Standard-SCSS extends standard, then disables rules that conflict with SCSS.
            let mut rules = HashMap::new();
            for &rule in STYLELINT_RECOMMENDED_RULES {
                rules.insert(
                    rule.to_string(),
                    RuleConfig {
                        severity: Some(Severity::Warning),
                        options: None,
                    },
                );
            }
            for &rule in STYLELINT_STANDARD_EXTRA_RULES {
                rules.insert(
                    rule.to_string(),
                    RuleConfig {
                        severity: Some(Severity::Warning),
                        options: None,
                    },
                );
            }
            // Disable rules that conflict with SCSS (inherited from recommended-scss)
            for &rule in &[
                "at-rule-no-unknown",
                "comment-no-empty",
                "no-duplicate-selectors",
                "no-invalid-position-at-import-rule",
            ] {
                rules.insert(
                    rule.to_string(),
                    RuleConfig {
                        severity: Some(Severity::Off),
                        options: None,
                    },
                );
            }
            Some(rules)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Config file search
// ---------------------------------------------------------------------------

/// Well-known config file names in priority order.
const CONFIG_FILENAMES: &[&str] = &[
    "gale.json",
    "gale.toml",
    ".stylelintrc",
    ".stylelintrc.json",
    ".stylelintrc.yml",
    ".stylelintrc.yaml",
    "stylelint.config.js",
    "stylelint.config.mjs",
    "stylelint.config.cjs",
    ".stylelintrc.js",
    ".stylelintrc.cjs",
];

/// Walk upward from `start_dir` looking for the first config file.
pub fn find_config(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        for name in CONFIG_FILENAMES {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        if !dir.pop() {
            return None;
        }
    }
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

/// Load and parse a config file at the given path.
pub fn load_config(path: &Path) -> Result<GaleConfig, ConfigError> {
    let contents = std::fs::read_to_string(path)?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();

    let base_dir = path.parent().unwrap_or(Path::new("."));
    let raw = parse_config_file(file_name, &contents)?;
    Ok(resolve_raw(raw, base_dir))
}

// ---------------------------------------------------------------------------
// JavaScript config parsing (module.exports = { ... })
// ---------------------------------------------------------------------------

/// Parse a JavaScript config file that uses `module.exports = { ... }`,
/// `exports = { ... }`, or `export default { ... }` with a static object literal.
///
/// This does NOT execute JavaScript — it extracts the object literal and converts
/// JS object syntax into valid JSON before deserializing.
fn parse_js_config(source: &str) -> Result<ConfigFile, ConfigError> {
    // Find the start of the exported object literal.
    let markers = [
        "module.exports",
        "exports",
        "export default",
    ];

    let mut obj_start: Option<usize> = None;

    for marker in &markers {
        if let Some(pos) = source.find(marker) {
            // Find the `=` after the marker (for module.exports / exports), or
            // no `=` for `export default`.
            let after_marker = pos + marker.len();
            let rest = &source[after_marker..];
            let brace_search = if *marker == "export default" {
                rest
            } else {
                // Skip past the `=`
                if let Some(eq_pos) = rest.find('=') {
                    &rest[eq_pos + 1..]
                } else {
                    continue;
                }
            };
            // Find the opening `{`
            if let Some(brace_offset) = brace_search.find('{') {
                let absolute_offset = source.len() - brace_search.len() + brace_offset;
                obj_start = Some(absolute_offset);
                break;
            }
        }
    }

    let start = obj_start.ok_or_else(|| {
        ConfigError::UnsupportedFormat(
            "no module.exports/export default object found in JS config".to_string(),
        )
    })?;

    // Extract from opening `{` to the matching `}`, respecting strings and comments.
    let extracted = extract_braced_object(&source[start..]).ok_or_else(|| {
        ConfigError::UnsupportedFormat("failed to extract object literal from JS config".to_string())
    })?;

    // Convert JS object syntax to JSON.
    let json = js_object_to_json(&extracted);

    serde_json::from_str::<ConfigFile>(&json).map_err(ConfigError::from)
}

/// Extract a brace-balanced substring starting from the opening `{`.
/// Returns the substring including the outer braces.
fn extract_braced_object(s: &str) -> Option<String> {
    let mut depth = 0i32;
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_template = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' && (in_single_quote || in_double_quote || in_template) {
            result.push(c);
            escape_next = true;
            continue;
        }

        // String handling
        if !in_double_quote && !in_template && c == '\'' {
            in_single_quote = !in_single_quote;
            result.push(c);
            continue;
        }
        if !in_single_quote && !in_template && c == '"' {
            in_double_quote = !in_double_quote;
            result.push(c);
            continue;
        }
        if !in_single_quote && !in_double_quote && c == '`' {
            in_template = !in_template;
            result.push(c);
            continue;
        }

        if in_single_quote || in_double_quote || in_template {
            result.push(c);
            continue;
        }

        // Line comments
        if c == '/' {
            if chars.peek() == Some(&'/') {
                // Skip until end of line
                for nc in chars.by_ref() {
                    if nc == '\n' {
                        break;
                    }
                }
                result.push(' '); // replace comment with space
                continue;
            }
            if chars.peek() == Some(&'*') {
                // Block comment — skip until */
                chars.next(); // consume *
                loop {
                    match chars.next() {
                        Some('*') if chars.peek() == Some(&'/') => {
                            chars.next(); // consume /
                            break;
                        }
                        None => break,
                        _ => {}
                    }
                }
                result.push(' ');
                continue;
            }
        }

        if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth -= 1;
        }

        result.push(c);

        if depth == 0 {
            return Some(result);
        }
    }

    None // unbalanced braces
}

/// Convert a JS object literal string to valid JSON.
///
/// Handles:
/// - Single-quoted strings → double-quoted
/// - Unquoted keys → double-quoted
/// - Trailing commas → removed
/// - Spread operator entries (`...foo`) → skipped
fn js_object_to_json(js: &str) -> String {
    // Step 1: Replace single-quoted strings with double-quoted strings.
    let s = convert_single_to_double_quotes(js);

    // Step 2: Quote unquoted keys.
    let s = quote_unquoted_keys(&s);

    // Step 3: Remove trailing commas.
    let s = remove_trailing_commas(&s);

    // Step 4: Remove spread entries.
    remove_spread_entries(&s)
}

/// Convert single-quoted strings to double-quoted strings.
/// Handles escaping: internal `'` becomes `\'` → `"` becomes itself,
/// and internal unescaped `"` gets escaped.
fn convert_single_to_double_quotes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_double_quote = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' && in_double_quote {
            result.push(c);
            escape_next = true;
            continue;
        }

        if c == '"' && !in_double_quote {
            // Not inside a double-quoted string, not starting one unless context says so
            // Check if we're about to start a double-quoted string
            in_double_quote = true;
            result.push('"');
            continue;
        }

        if c == '"' && in_double_quote {
            in_double_quote = false;
            result.push('"');
            continue;
        }

        if in_double_quote {
            if c == '\\' {
                escape_next = true;
            }
            result.push(c);
            continue;
        }

        // Outside any string context
        if c == '\'' {
            // Start of single-quoted string — collect until closing '
            result.push('"');
            loop {
                match chars.next() {
                    None => break,
                    Some('\\') => {
                        // Next char is escaped
                        if let Some(ec) = chars.next() {
                            if ec == '\'' {
                                // Escaped single quote — just emit the quote
                                result.push('\'');
                            } else {
                                result.push('\\');
                                result.push(ec);
                            }
                        }
                    }
                    Some('\'') => {
                        // End of single-quoted string
                        result.push('"');
                        break;
                    }
                    Some('"') => {
                        // Unescaped double quote inside single-quoted string — escape it
                        result.push('\\');
                        result.push('"');
                    }
                    Some(ch) => result.push(ch),
                }
            }
            continue;
        }

        result.push(c);
    }

    result
}

/// Add double quotes around unquoted object keys.
/// An unquoted key looks like `identifier:` or `identifier :` at the start
/// of a line or after `{` or `,`.
fn quote_unquoted_keys(s: &str) -> String {
    // Use a regex-like approach: scan for patterns like `word :` or `word:`
    // that are not inside strings.
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;
    let mut escape_next = false;

    while i < len {
        let c = chars[i];

        if escape_next {
            result.push(c);
            escape_next = false;
            i += 1;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            i += 1;
            continue;
        }

        if c == '"' {
            in_string = !in_string;
            result.push(c);
            i += 1;
            continue;
        }

        if in_string {
            result.push(c);
            i += 1;
            continue;
        }

        // Check if we're at the start of an unquoted key.
        // An unquoted key is an identifier ([a-zA-Z_$][a-zA-Z0-9_$-]*)
        // followed by optional whitespace and a `:`.
        if c.is_ascii_alphabetic() || c == '_' || c == '$' {
            // Check what came before — should be start, `{`, `,`, or whitespace/newline after one of those.
            let before_is_valid = {
                let trimmed_before = result.trim_end();
                trimmed_before.is_empty()
                    || trimmed_before.ends_with('{')
                    || trimmed_before.ends_with(',')
                    || trimmed_before.ends_with('\n')
            };

            if before_is_valid {
                // Collect the identifier
                let key_start = i;
                while i < len
                    && (chars[i].is_ascii_alphanumeric()
                        || chars[i] == '_'
                        || chars[i] == '$'
                        || chars[i] == '-')
                {
                    i += 1;
                }
                let key: String = chars[key_start..i].iter().collect();

                // Skip whitespace
                let ws_start = i;
                while i < len && chars[i].is_ascii_whitespace() {
                    i += 1;
                }

                // Check for `:`
                if i < len && chars[i] == ':' {
                    // It's an unquoted key — quote it
                    result.push('"');
                    result.push_str(&key);
                    result.push('"');
                    // Push any whitespace that was between key and `:`
                    let ws: String = chars[ws_start..i].iter().collect();
                    result.push_str(&ws);
                    result.push(':');
                    i += 1; // skip the `:`
                    continue;
                } else {
                    // Not a key — push as-is
                    result.push_str(&key);
                    let ws: String = chars[ws_start..i].iter().collect();
                    result.push_str(&ws);
                    continue;
                }
            }
        }

        result.push(c);
        i += 1;
    }
    result
}

/// Remove trailing commas before `}` or `]`.
fn remove_trailing_commas(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;
    let mut escape_next = false;

    while i < len {
        let c = chars[i];

        if escape_next {
            result.push(c);
            escape_next = false;
            i += 1;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            i += 1;
            continue;
        }

        if c == '"' {
            in_string = !in_string;
            result.push(c);
            i += 1;
            continue;
        }

        if in_string {
            result.push(c);
            i += 1;
            continue;
        }

        if c == ',' {
            // Look ahead past whitespace for `}` or `]`
            let mut j = i + 1;
            while j < len && chars[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < len && (chars[j] == '}' || chars[j] == ']') {
                // Skip this comma (trailing comma)
                i += 1;
                continue;
            }
        }

        result.push(c);
        i += 1;
    }

    result
}

/// Remove spread operator entries like `...something` from object/array literals.
fn remove_spread_entries(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;
    let mut escape_next = false;

    while i < len {
        let c = chars[i];

        if escape_next {
            result.push(c);
            escape_next = false;
            i += 1;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            i += 1;
            continue;
        }

        if c == '"' {
            in_string = !in_string;
            result.push(c);
            i += 1;
            continue;
        }

        if in_string {
            result.push(c);
            i += 1;
            continue;
        }

        // Detect `...identifier`
        if c == '.'
            && i + 2 < len
            && chars[i + 1] == '.'
            && chars[i + 2] == '.'
        {
            // Skip `...` and the following identifier
            i += 3;
            while i < len
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '$')
            {
                i += 1;
            }
            // Also skip a trailing comma if present
            // Skip whitespace first
            while i < len && chars[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < len && chars[i] == ',' {
                i += 1;
            }
            continue;
        }

        result.push(c);
        i += 1;
    }

    result
}

fn parse_config_file(file_name: &str, contents: &str) -> Result<ConfigFile, ConfigError> {
    if file_name.ends_with(".json") || file_name == "gale.json" {
        Ok(serde_json::from_str(contents)?)
    } else if file_name.ends_with(".toml") || file_name == "gale.toml" {
        Ok(toml::from_str(contents)?)
    } else if file_name.ends_with(".yml") || file_name.ends_with(".yaml") {
        Ok(serde_yaml::from_str(contents)?)
    } else if file_name.ends_with(".js")
        || file_name.ends_with(".mjs")
        || file_name.ends_with(".cjs")
    {
        parse_js_config(contents)
    } else if file_name == ".stylelintrc" {
        // Try JSON first, fall back to YAML.
        serde_json::from_str(contents)
            .map_err(ConfigError::from)
            .or_else(|_| serde_yaml::from_str(contents).map_err(ConfigError::from))
    } else {
        Err(ConfigError::UnsupportedFormat(file_name.to_string()))
    }
}

/// Read a file and parse it as a [`ConfigFile`], inferring format from the file name.
fn load_config_file(path: &Path) -> Result<ConfigFile, ConfigError> {
    let contents = std::fs::read_to_string(path)?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();

    // For files without a recognised extension (e.g. bare `.stylelintrc`),
    // or JSON files, try JSON first then YAML as a fallback.
    if file_name.ends_with(".json") {
        Ok(serde_json::from_str(&contents)?)
    } else if file_name.ends_with(".toml") {
        Ok(toml::from_str(&contents)?)
    } else if file_name.ends_with(".yml") || file_name.ends_with(".yaml") {
        Ok(serde_yaml::from_str(&contents)?)
    } else if file_name.ends_with(".js")
        || file_name.ends_with(".mjs")
        || file_name.ends_with(".cjs")
    {
        parse_js_config(&contents)
    } else {
        // Unknown extension — try JSON, then YAML.
        serde_json::from_str(&contents)
            .map_err(ConfigError::from)
            .or_else(|_| serde_yaml::from_str(&contents).map_err(ConfigError::from))
    }
}

// ---------------------------------------------------------------------------
// npm / node_modules resolution
// ---------------------------------------------------------------------------

/// Walk upward from `start_dir` looking for a `node_modules` directory.
fn find_node_modules(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let nm = dir.join("node_modules");
        if nm.is_dir() {
            return Some(nm);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Try to load a [`ConfigFile`] from an npm package installed in `node_modules`.
fn resolve_npm_config(package_name: &str, base_dir: &Path) -> Option<ConfigFile> {
    let node_modules = find_node_modules(base_dir)?;
    let pkg_dir = node_modules.join(package_name);

    if !pkg_dir.is_dir() {
        return None;
    }

    // 1. Try package.json "main" field
    let pkg_json_path = pkg_dir.join("package.json");
    if let Ok(pkg_json) = std::fs::read_to_string(&pkg_json_path)
        && let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&pkg_json)
        && let Some(main) = pkg.get("main").and_then(|m| m.as_str())
    {
        let main_path = pkg_dir.join(main);
        if let Ok(config) = load_config_file(&main_path) {
            return Some(config);
        }
    }

    // 2. Try common config file names
    for name in &["index.json", "index.js", ".stylelintrc.json", ".stylelintrc"] {
        let path = pkg_dir.join(name);
        if let Ok(config) = load_config_file(&path) {
            return Some(config);
        }
    }

    None
}

/// Resolve a relative path (starting with `./` or `../`) to a [`ConfigFile`].
fn resolve_relative_config(rel_path: &str, base_dir: &Path) -> Option<ConfigFile> {
    let path = base_dir.join(rel_path);
    load_config_file(&path).ok()
}

/// Recursively collect rules from a list of `extends` entries, with cycle detection.
fn collect_rules_from_extends(
    extends: &[String],
    base_dir: &Path,
    visited: &mut HashSet<String>,
) -> HashMap<String, RuleConfig> {
    let mut rules = HashMap::new();

    for preset_name in extends {
        if visited.contains(preset_name) {
            continue; // cycle detection
        }
        visited.insert(preset_name.clone());

        // Try built-in presets first (gale:*, stylelint-config-*, etc.)
        if let Some(preset_rules) = resolve_preset(preset_name) {
            rules.extend(preset_rules);
        } else if preset_name.starts_with("gale:") {
            // Unknown gale: preset — warn and skip.
            eprintln!("warning: unknown preset '{preset_name}', skipping");
        } else {
            // Resolve as relative path or npm package
            let config = if preset_name.starts_with("./") || preset_name.starts_with("../") {
                resolve_relative_config(preset_name, base_dir)
            } else {
                resolve_npm_config(preset_name, base_dir)
            };

            if let Some(config) = config {
                // Recursively resolve this config's extends first
                if let Some(ref sub_extends) = config.extends {
                    let sub_base = if preset_name.starts_with("./")
                        || preset_name.starts_with("../")
                    {
                        base_dir
                            .join(preset_name)
                            .parent()
                            .unwrap_or(base_dir)
                            .to_path_buf()
                    } else {
                        find_node_modules(base_dir)
                            .map(|nm| nm.join(preset_name))
                            .unwrap_or_else(|| base_dir.to_path_buf())
                    };
                    let sub_rules =
                        collect_rules_from_extends(sub_extends, &sub_base, visited);
                    rules.extend(sub_rules);
                }
                // Then apply this config's own rules (later configs win)
                for (name, value) in config.rules.unwrap_or_default() {
                    let resolved = value.resolve();
                    if resolved.severity == Some(Severity::Off) {
                        rules.remove(&name);
                    } else {
                        rules.insert(name, resolved);
                    }
                }
            } else {
                eprintln!("warning: could not resolve config '{preset_name}', skipping");
            }
        }
    }

    rules
}

/// Convert a raw [`ConfigFile`] into the resolved [`GaleConfig`].
///
/// `base_dir` is used when resolving `extends` that reference npm packages or
/// relative paths.
fn resolve_raw(raw: ConfigFile, base_dir: &Path) -> GaleConfig {
    // 1. Start with rules from extended presets / npm configs (in order).
    let mut rules: HashMap<String, RuleConfig> = HashMap::new();
    if let Some(ref extends) = raw.extends {
        let mut visited = HashSet::new();
        rules = collect_rules_from_extends(extends, base_dir, &mut visited);
    }

    // 2. Overlay user rules on top — user always wins.
    for (name, value) in raw.rules.unwrap_or_default() {
        let resolved = value.resolve();
        // If the user set a rule to Off, remove it from the map entirely
        // so the linter doesn't run it at all.
        if resolved.severity == Some(Severity::Off) {
            rules.remove(&name);
        } else {
            rules.insert(name, resolved);
        }
    }

    let ignore_patterns = raw.ignore_patterns.unwrap_or_default();

    let formatter = raw
        .formatter
        .as_deref()
        .map(|s| match s.to_lowercase().as_str() {
            "json" => FormatterType::Json,
            "compact" => FormatterType::Compact,
            _ => FormatterType::Text,
        })
        .unwrap_or_default();

    // 3. Resolve overrides.
    let overrides = if let Some(raw_overrides) = raw.overrides {
        raw_overrides
            .into_iter()
            .filter_map(|ov| {
                let file_patterns = ov.files.unwrap_or_default();
                if file_patterns.is_empty() {
                    return None;
                }

                // Start with rules from the override's extends.
                let mut ov_rules: HashMap<String, RuleConfig> = HashMap::new();
                if let Some(ref extends) = ov.extends {
                    let mut visited = HashSet::new();
                    ov_rules = collect_rules_from_extends(extends, base_dir, &mut visited);
                }

                // Overlay the override's own rules on top.
                for (name, value) in ov.rules.unwrap_or_default() {
                    let resolved = value.resolve();
                    if resolved.severity == Some(Severity::Off) {
                        // For overrides, keep Off entries so they can remove
                        // base rules when applied per-file.
                        ov_rules.insert(name, resolved);
                    } else {
                        ov_rules.insert(name, resolved);
                    }
                }

                Some(ResolvedOverride::new(file_patterns, ov_rules))
            })
            .collect()
    } else {
        Vec::new()
    };

    GaleConfig {
        rules,
        ignore_patterns,
        formatter,
        overrides,
    }
}

// ---------------------------------------------------------------------------
// High-level convenience
// ---------------------------------------------------------------------------

/// Find a config file starting from `start_dir`, load it, or return defaults.
pub fn resolve_config(start_dir: &Path) -> GaleConfig {
    match find_config(start_dir) {
        Some(path) => load_config(&path).unwrap_or_default(),
        None => GaleConfig::default(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = GaleConfig::default();
        assert!(cfg.rules.is_empty());
        assert!(cfg.ignore_patterns.is_empty());
        assert_eq!(cfg.formatter, FormatterType::Text);
    }

    #[test]
    fn parse_rule_bool() {
        let v = RuleConfigValue::Bool(true);
        let r = v.resolve();
        assert_eq!(r.severity, Some(Severity::Error));
        assert!(r.options.is_none());

        let v = RuleConfigValue::Bool(false);
        let r = v.resolve();
        assert_eq!(r.severity, Some(Severity::Off));
    }

    #[test]
    fn parse_rule_string() {
        let v = RuleConfigValue::Severity("warning".to_string());
        let r = v.resolve();
        assert_eq!(r.severity, Some(Severity::Warning));
    }

    #[test]
    fn parse_rule_array() {
        let v: RuleConfigValue = serde_json::from_str(r#"["error", {"max": 3}]"#).unwrap();
        let r = v.resolve();
        assert_eq!(r.severity, Some(Severity::Error));
        assert!(r.options.is_some());
    }

    #[test]
    fn parse_json_config() {
        let json = r#"{
            "rules": {
                "color-no-invalid-hex": true,
                "block-no-empty": "warning",
                "declaration-no-important": ["error", {"severity": "strict"}]
            },
            "ignorePatterns": ["dist/**", "node_modules/**"],
            "formatter": "json"
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        assert_eq!(cfg.rules.len(), 3);
        assert_eq!(cfg.ignore_patterns.len(), 2);
        assert_eq!(cfg.formatter, FormatterType::Json);
    }

    #[test]
    fn parse_toml_config() {
        let t = r#"
            formatter = "compact"

            [rules]
            color-no-invalid-hex = true
            block-no-empty = "off"
        "#;
        let raw: ConfigFile = toml::from_str(t).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        // block-no-empty is "off" so it gets removed; only color-no-invalid-hex remains.
        assert_eq!(cfg.rules.len(), 1);
        assert!(cfg.rules.contains_key("color-no-invalid-hex"));
        assert_eq!(cfg.formatter, FormatterType::Compact);
    }

    #[test]
    fn parse_yaml_config() {
        let yaml = r#"
rules:
  color-no-invalid-hex: true
  block-no-empty: warning
ignorePatterns:
  - "dist/**"
formatter: text
"#;
        let raw: ConfigFile = serde_yaml::from_str(yaml).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        assert_eq!(cfg.rules.len(), 2);
        assert_eq!(cfg.ignore_patterns.len(), 1);
        assert_eq!(cfg.formatter, FormatterType::Text);
    }

    // -----------------------------------------------------------------------
    // Preset / extends tests
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_recommended_preset() {
        let preset = resolve_preset("gale:recommended").unwrap();
        // Should contain all error + warning rules.
        assert_eq!(
            preset.len(),
            RECOMMENDED_ERROR_RULES.len() + RECOMMENDED_WARNING_RULES.len()
        );
        // Spot-check severities.
        assert_eq!(
            preset["block-no-empty"].severity,
            Some(Severity::Error)
        );
        assert_eq!(
            preset["color-hex-length"].severity,
            Some(Severity::Warning)
        );
    }

    #[test]
    fn resolve_all_preset() {
        let preset = resolve_preset("gale:all").unwrap();
        assert_eq!(preset.len(), ALL_RULE_NAMES.len());
        // Every rule should be warning.
        for rule_cfg in preset.values() {
            assert_eq!(rule_cfg.severity, Some(Severity::Warning));
        }
    }

    #[test]
    fn unknown_preset_returns_none() {
        assert!(resolve_preset("gale:nonexistent").is_none());
        assert!(resolve_preset("stylelint:recommended").is_none());
    }

    #[test]
    fn extends_string_json() {
        let json = r#"{ "extends": "gale:recommended" }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        assert_eq!(raw.extends, Some(vec!["gale:recommended".to_string()]));
    }

    #[test]
    fn extends_array_json() {
        let json = r#"{ "extends": ["gale:recommended", "gale:all"] }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        assert_eq!(
            raw.extends,
            Some(vec![
                "gale:recommended".to_string(),
                "gale:all".to_string()
            ])
        );
    }

    #[test]
    fn extends_loads_preset_rules() {
        let json = r#"{ "extends": "gale:recommended" }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        // Should have all recommended rules.
        assert!(cfg.rules.contains_key("block-no-empty"));
        assert!(cfg.rules.contains_key("color-hex-length"));
        assert_eq!(
            cfg.rules.len(),
            RECOMMENDED_ERROR_RULES.len() + RECOMMENDED_WARNING_RULES.len()
        );
    }

    #[test]
    fn user_rules_override_preset() {
        let json = r#"{
            "extends": "gale:recommended",
            "rules": {
                "block-no-empty": "warning"
            }
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        // Preset sets block-no-empty to error, user overrides to warning.
        assert_eq!(
            cfg.rules["block-no-empty"].severity,
            Some(Severity::Warning)
        );
    }

    #[test]
    fn user_can_disable_preset_rule() {
        let json = r#"{
            "extends": "gale:recommended",
            "rules": {
                "block-no-empty": "off"
            }
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        // The rule should be removed entirely.
        assert!(!cfg.rules.contains_key("block-no-empty"));
    }

    #[test]
    fn user_can_disable_preset_rule_with_false() {
        let json = r#"{
            "extends": "gale:recommended",
            "rules": {
                "block-no-empty": false
            }
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        assert!(!cfg.rules.contains_key("block-no-empty"));
    }

    #[test]
    fn unknown_preset_is_skipped_gracefully() {
        let json = r#"{
            "extends": ["gale:nonexistent", "gale:recommended"],
            "rules": {}
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        // Should still have recommended rules despite the unknown preset.
        assert!(cfg.rules.contains_key("block-no-empty"));
    }

    #[test]
    fn later_preset_overrides_earlier() {
        // gale:recommended sets block-no-empty to error.
        // gale:all sets everything to warning.
        // gale:all comes second, so it should win.
        let json = r#"{ "extends": ["gale:recommended", "gale:all"] }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        assert_eq!(
            cfg.rules["block-no-empty"].severity,
            Some(Severity::Warning)
        );
    }

    #[test]
    fn extends_toml_string() {
        let t = r#"
            extends = "gale:recommended"

            [rules]
            block-no-empty = "off"
        "#;
        let raw: ConfigFile = toml::from_str(t).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        assert!(!cfg.rules.contains_key("block-no-empty"));
        // Other recommended rules should still be present.
        assert!(cfg.rules.contains_key("color-no-invalid-hex"));
    }

    #[test]
    fn extends_toml_array() {
        let t = r#"
            extends = ["gale:recommended"]
        "#;
        let raw: ConfigFile = toml::from_str(t).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        assert!(cfg.rules.contains_key("block-no-empty"));
    }

    #[test]
    fn no_extends_works_as_before() {
        let json = r#"{
            "rules": {
                "color-no-invalid-hex": true
            }
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        assert_eq!(cfg.rules.len(), 1);
        assert!(cfg.rules.contains_key("color-no-invalid-hex"));
    }

    // -----------------------------------------------------------------------
    // JavaScript config parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn js_config_module_exports_single_quoted_keys() {
        let js = r#"
'use strict';

module.exports = {
  'extends': ['stylelint-config-standard'],
  'rules': {
    'alpha-value-notation': null,
    'color-named': 'never',
  }
};
"#;
        let raw = parse_js_config(js).unwrap();
        assert_eq!(
            raw.extends,
            Some(vec!["stylelint-config-standard".to_string()])
        );
        let rules = raw.rules.unwrap();
        assert!(rules.contains_key("color-named"));
    }

    #[test]
    fn js_config_trailing_commas() {
        let js = r#"
module.exports = {
  "rules": {
    "block-no-empty": true,
    "color-no-invalid-hex": true,
  },
};
"#;
        let raw = parse_js_config(js).unwrap();
        let rules = raw.rules.unwrap();
        assert_eq!(rules.len(), 2);
        assert!(rules.contains_key("block-no-empty"));
        assert!(rules.contains_key("color-no-invalid-hex"));
    }

    #[test]
    fn js_config_null_values() {
        let js = r#"
module.exports = {
  'rules': {
    'alpha-value-notation': null,
    'declaration-no-important': true,
  }
};
"#;
        let raw = parse_js_config(js).unwrap();
        let rules = raw.rules.unwrap();
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn js_config_array_values() {
        let js = r#"
module.exports = {
  'rules': {
    'font-weight-notation': ['numeric', { 'ignore': ['relative'] }],
    'selector-max-id': 0,
  }
};
"#;
        let raw = parse_js_config(js).unwrap();
        let rules = raw.rules.unwrap();
        assert!(rules.contains_key("font-weight-notation"));
        assert!(rules.contains_key("selector-max-id"));
    }

    #[test]
    fn js_config_nested_objects() {
        let js = r#"
module.exports = {
  'rules': {
    'font-weight-notation': ['numeric', { 'ignore': ['relative'] }],
    'color-named': 'never',
  }
};
"#;
        let raw = parse_js_config(js).unwrap();
        let rules = raw.rules.unwrap();
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn js_config_export_default() {
        let js = r#"
export default {
  "extends": ["stylelint-config-standard"],
  "rules": {
    "block-no-empty": true
  }
};
"#;
        let raw = parse_js_config(js).unwrap();
        assert_eq!(
            raw.extends,
            Some(vec!["stylelint-config-standard".to_string()])
        );
        let rules = raw.rules.unwrap();
        assert!(rules.contains_key("block-no-empty"));
    }

    #[test]
    fn js_config_unquoted_keys() {
        let js = r#"
module.exports = {
  extends: ['stylelint-config-standard'],
  rules: {
    'block-no-empty': true,
  }
};
"#;
        let raw = parse_js_config(js).unwrap();
        assert_eq!(
            raw.extends,
            Some(vec!["stylelint-config-standard".to_string()])
        );
    }

    #[test]
    fn js_config_with_comments() {
        let js = r#"
// Main config
module.exports = {
  /* extends from standard */
  'extends': ['stylelint-config-standard'],
  'rules': {
    // Disable this rule
    'block-no-empty': true,
  }
};
"#;
        let raw = parse_js_config(js).unwrap();
        assert!(raw.extends.is_some());
    }

    #[test]
    fn js_config_with_spread_skipped() {
        let js = r#"
module.exports = {
  ...baseConfig,
  'rules': {
    'block-no-empty': true,
  }
};
"#;
        let raw = parse_js_config(js).unwrap();
        let rules = raw.rules.unwrap();
        assert!(rules.contains_key("block-no-empty"));
    }

    #[test]
    fn js_config_no_exports_returns_error() {
        let js = r#"
const config = {
  rules: { 'block-no-empty': true }
};
"#;
        assert!(parse_js_config(js).is_err());
    }

    #[test]
    fn js_config_parse_config_file_dispatches() {
        let js = r#"
module.exports = {
  'rules': {
    'block-no-empty': true,
  }
};
"#;
        let raw = parse_config_file("stylelint.config.js", js).unwrap();
        let rules = raw.rules.unwrap();
        assert!(rules.contains_key("block-no-empty"));

        // Also works for .mjs
        let raw2 = parse_config_file("stylelint.config.mjs", js).unwrap();
        assert!(raw2.rules.is_some());

        // Also works for .cjs
        let raw3 = parse_config_file("stylelint.config.cjs", js).unwrap();
        assert!(raw3.rules.is_some());
    }

    #[test]
    fn js_config_full_realistic_example() {
        let js = r#"
'use strict';

module.exports = {
  'extends': ['stylelint-config-standard'],
  'rules': {
    'alpha-value-notation': null,
    'color-named': 'never',
    'declaration-no-important': true,
    'font-weight-notation': ['numeric', { 'ignore': ['relative'] }],
    'selector-max-id': 0,
  }
};
"#;
        let raw = parse_js_config(js).unwrap();
        // Verify parsing succeeded and extracted the right structure.
        assert_eq!(
            raw.extends,
            Some(vec!["stylelint-config-standard".to_string()])
        );
        let rules = raw.rules.unwrap();
        assert_eq!(rules.len(), 5);
        assert!(rules.contains_key("alpha-value-notation"));
        assert!(rules.contains_key("color-named"));
        assert!(rules.contains_key("declaration-no-important"));
        assert!(rules.contains_key("font-weight-notation"));
        assert!(rules.contains_key("selector-max-id"));
    }

    // -----------------------------------------------------------------------
    // Override tests
    // -----------------------------------------------------------------------

    #[test]
    fn resolved_override_matches_glob() {
        let ov = ResolvedOverride::new(
            vec!["**/*.scss".to_string()],
            HashMap::new(),
        );
        assert!(ov.matches("src/styles/main.scss"));
        assert!(ov.matches("main.scss"));
        assert!(!ov.matches("main.css"));
        assert!(!ov.matches("main.less"));
    }

    #[test]
    fn resolved_override_matches_multiple_patterns() {
        let ov = ResolvedOverride::new(
            vec!["**/*.scss".to_string(), "**/*.less".to_string()],
            HashMap::new(),
        );
        assert!(ov.matches("main.scss"));
        assert!(ov.matches("main.less"));
        assert!(!ov.matches("main.css"));
    }

    #[test]
    fn overrides_parsed_from_json() {
        let json = r#"{
            "extends": "gale:recommended",
            "overrides": [
                {
                    "files": "**/*.scss",
                    "rules": {
                        "no-duplicate-selectors": null,
                        "comment-no-empty": null
                    }
                }
            ]
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        assert!(raw.overrides.is_some());
        let overrides = raw.overrides.unwrap();
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].files, Some(vec!["**/*.scss".to_string()]));
        let rules = overrides[0].rules.as_ref().unwrap();
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn overrides_files_string_or_vec() {
        // Single string
        let json = r#"{ "overrides": [{ "files": "**/*.scss", "rules": {} }] }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let overrides = raw.overrides.unwrap();
        assert_eq!(overrides[0].files, Some(vec!["**/*.scss".to_string()]));

        // Array
        let json = r#"{ "overrides": [{ "files": ["**/*.scss", "**/*.less"], "rules": {} }] }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let overrides = raw.overrides.unwrap();
        assert_eq!(
            overrides[0].files,
            Some(vec!["**/*.scss".to_string(), "**/*.less".to_string()])
        );
    }

    #[test]
    fn overrides_with_extends() {
        let json = r#"{
            "overrides": [
                {
                    "files": "**/*.scss",
                    "extends": "stylelint-config-standard-scss"
                }
            ]
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let overrides = raw.overrides.unwrap();
        assert_eq!(
            overrides[0].extends,
            Some(vec!["stylelint-config-standard-scss".to_string()])
        );
    }

    #[test]
    fn resolve_config_with_overrides() {
        let json = r#"{
            "extends": "gale:recommended",
            "overrides": [
                {
                    "files": "**/*.scss",
                    "rules": {
                        "no-duplicate-selectors": null,
                        "comment-no-empty": null
                    }
                }
            ]
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        assert_eq!(cfg.overrides.len(), 1);
        assert!(cfg.overrides[0].matches("main.scss"));
        assert!(!cfg.overrides[0].matches("main.css"));
    }

    #[test]
    fn rules_for_file_applies_overrides() {
        let json = r#"{
            "extends": "gale:recommended",
            "overrides": [
                {
                    "files": "**/*.scss",
                    "rules": {
                        "no-duplicate-selectors": null,
                        "comment-no-empty": null
                    }
                }
            ]
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));

        // CSS files should have the base rules unchanged.
        let css_rules = cfg.rules_for_file("main.css");
        assert!(css_rules.contains_key("no-duplicate-selectors"));

        // SCSS files should have the overridden rules removed.
        let scss_rules = cfg.rules_for_file("main.scss");
        assert!(!scss_rules.contains_key("no-duplicate-selectors"));
        assert!(!scss_rules.contains_key("comment-no-empty"));
        // But other base rules should still be present.
        assert!(scss_rules.contains_key("block-no-empty"));
    }

    #[test]
    fn rules_for_file_no_overrides_returns_base() {
        let json = r#"{
            "extends": "gale:recommended"
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));

        let rules = cfg.rules_for_file("main.scss");
        assert_eq!(rules, cfg.rules);
    }

    #[test]
    fn multiple_overrides_applied_in_order() {
        let json = r#"{
            "rules": {
                "block-no-empty": true,
                "color-no-invalid-hex": true
            },
            "overrides": [
                {
                    "files": "**/*.scss",
                    "rules": {
                        "block-no-empty": "warning"
                    }
                },
                {
                    "files": "**/*.scss",
                    "rules": {
                        "block-no-empty": "off"
                    }
                }
            ]
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));

        // The second override should win: block-no-empty should be removed.
        let scss_rules = cfg.rules_for_file("main.scss");
        assert!(!scss_rules.contains_key("block-no-empty"));
        // color-no-invalid-hex should remain untouched.
        assert!(scss_rules.contains_key("color-no-invalid-hex"));
    }

    #[test]
    fn override_adds_new_rule() {
        let json = r#"{
            "rules": {
                "block-no-empty": true
            },
            "overrides": [
                {
                    "files": "**/*.scss",
                    "rules": {
                        "color-no-invalid-hex": "warning"
                    }
                }
            ]
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));

        // CSS file should only have the base rule.
        let css_rules = cfg.rules_for_file("main.css");
        assert_eq!(css_rules.len(), 1);

        // SCSS file should have both base + override rule.
        let scss_rules = cfg.rules_for_file("main.scss");
        assert_eq!(scss_rules.len(), 2);
        assert!(scss_rules.contains_key("block-no-empty"));
        assert!(scss_rules.contains_key("color-no-invalid-hex"));
        assert_eq!(
            scss_rules["color-no-invalid-hex"].severity,
            Some(Severity::Warning)
        );
    }

    #[test]
    fn override_with_extends_preset() {
        let json = r#"{
            "rules": {
                "block-no-empty": true
            },
            "overrides": [
                {
                    "files": "**/*.scss",
                    "extends": "stylelint-config-recommended-scss",
                    "rules": {
                        "no-duplicate-selectors": null
                    }
                }
            ]
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));

        // SCSS file should have rules from the preset minus the null override.
        let scss_rules = cfg.rules_for_file("main.scss");
        assert!(!scss_rules.contains_key("no-duplicate-selectors"));
        // Should have rules from the preset (at-rule-no-unknown is off in recommended-scss).
        assert!(!scss_rules.contains_key("at-rule-no-unknown"));
        // Should still have the base rule.
        assert!(scss_rules.contains_key("block-no-empty"));
    }

    #[test]
    fn override_empty_files_skipped() {
        let json = r#"{
            "rules": {
                "block-no-empty": true
            },
            "overrides": [
                {
                    "rules": {
                        "block-no-empty": "off"
                    }
                }
            ]
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));

        // Override with no files should be skipped entirely.
        assert_eq!(cfg.overrides.len(), 0);
        let rules = cfg.rules_for_file("main.css");
        assert!(rules.contains_key("block-no-empty"));
    }

    #[test]
    fn override_has_overrides_flag() {
        let json_with = r#"{
            "overrides": [{ "files": "**/*.scss", "rules": {} }]
        }"#;
        let raw: ConfigFile = serde_json::from_str(json_with).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        assert!(cfg.has_overrides());

        let json_without = r#"{ "rules": {} }"#;
        let raw: ConfigFile = serde_json::from_str(json_without).unwrap();
        let cfg = resolve_raw(raw, Path::new("."));
        assert!(!cfg.has_overrides());
    }

    #[test]
    fn yaml_config_with_overrides() {
        let yaml = r#"
extends: gale:recommended
overrides:
  - files: "**/*.scss"
    rules:
      no-duplicate-selectors: null
      comment-no-empty: null
"#;
        let raw: ConfigFile = serde_yaml::from_str(yaml).unwrap();
        assert!(raw.overrides.is_some());
        let cfg = resolve_raw(raw, Path::new("."));
        assert_eq!(cfg.overrides.len(), 1);

        let scss_rules = cfg.rules_for_file("main.scss");
        assert!(!scss_rules.contains_key("no-duplicate-selectors"));
        assert!(!scss_rules.contains_key("comment-no-empty"));
    }
}
