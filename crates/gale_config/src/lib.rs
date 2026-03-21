use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

/// The fully-resolved configuration used by the linter at runtime.
#[derive(Debug, Clone)]
pub struct GaleConfig {
    pub rules: HashMap<String, RuleConfig>,
    pub ignore_patterns: Vec<String>,
    pub formatter: FormatterType,
}

impl Default for GaleConfig {
    fn default() -> Self {
        Self {
            rules: HashMap::new(),
            ignore_patterns: Vec::new(),
            formatter: FormatterType::Text,
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
}

/// A serde-friendly enum matching Stylelint's flexible rule value format.
///
/// Accepts any of:
/// - `true` / `false`  (boolean — true means error, false means off)
/// - `"error"` / `"warning"` / `"off"` (string severity)
/// - `["error", { ...options }]` (tuple of severity + options)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleConfigValue {
    /// Boolean shorthand: `true` → Error, `false` → Off.
    Bool(bool),
    /// String severity: `"error"`, `"warning"`, `"off"`.
    Severity(String),
    /// Array form: `["error", { ...options }]`.
    Array(Vec<serde_json::Value>),
}

impl RuleConfigValue {
    /// Convert the raw config value into a resolved [`RuleConfig`].
    pub fn resolve(&self) -> RuleConfig {
        match self {
            RuleConfigValue::Bool(b) => RuleConfig {
                severity: Some(if *b { Severity::Error } else { Severity::Off }),
                options: None,
            },
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

    let raw = parse_config_file(file_name, &contents)?;
    Ok(resolve_raw(raw))
}

fn parse_config_file(file_name: &str, contents: &str) -> Result<ConfigFile, ConfigError> {
    if file_name.ends_with(".json") || file_name == "gale.json" {
        Ok(serde_json::from_str(contents)?)
    } else if file_name.ends_with(".toml") || file_name == "gale.toml" {
        Ok(toml::from_str(contents)?)
    } else if file_name.ends_with(".yml") || file_name.ends_with(".yaml") {
        Ok(serde_yaml::from_str(contents)?)
    } else if file_name == ".stylelintrc" {
        // Try JSON first, fall back to YAML.
        serde_json::from_str(contents)
            .map_err(ConfigError::from)
            .or_else(|_| serde_yaml::from_str(contents).map_err(ConfigError::from))
    } else {
        Err(ConfigError::UnsupportedFormat(file_name.to_string()))
    }
}

/// Convert a raw [`ConfigFile`] into the resolved [`GaleConfig`].
fn resolve_raw(raw: ConfigFile) -> GaleConfig {
    // 1. Start with rules from extended presets (in order).
    let mut rules: HashMap<String, RuleConfig> = HashMap::new();
    if let Some(extends) = &raw.extends {
        for preset_name in extends {
            if let Some(preset_rules) = resolve_preset(preset_name) {
                // Later presets override earlier ones.
                rules.extend(preset_rules);
            } else {
                eprintln!("warning: unknown preset '{preset_name}', skipping");
            }
        }
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

    GaleConfig {
        rules,
        ignore_patterns,
        formatter,
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
        let cfg = resolve_raw(raw);
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
        let cfg = resolve_raw(raw);
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
        let cfg = resolve_raw(raw);
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
        let cfg = resolve_raw(raw);
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
        let cfg = resolve_raw(raw);
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
        let cfg = resolve_raw(raw);
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
        let cfg = resolve_raw(raw);
        assert!(!cfg.rules.contains_key("block-no-empty"));
    }

    #[test]
    fn unknown_preset_is_skipped_gracefully() {
        let json = r#"{
            "extends": ["gale:nonexistent", "gale:recommended"],
            "rules": {}
        }"#;
        let raw: ConfigFile = serde_json::from_str(json).unwrap();
        let cfg = resolve_raw(raw);
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
        let cfg = resolve_raw(raw);
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
        let cfg = resolve_raw(raw);
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
        let cfg = resolve_raw(raw);
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
        let cfg = resolve_raw(raw);
        assert_eq!(cfg.rules.len(), 1);
        assert!(cfg.rules.contains_key("color-no-invalid-hex"));
    }
}
