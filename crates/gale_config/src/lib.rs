use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

/// What is actually stored in a config file on disk.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFile {
    pub rules: Option<HashMap<String, RuleConfigValue>>,
    pub ignore_patterns: Option<Vec<String>>,
    pub formatter: Option<String>,
    /// Reserved for future use — list of shared configs to extend.
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
    let rules = raw
        .rules
        .unwrap_or_default()
        .into_iter()
        .map(|(name, value)| (name, value.resolve()))
        .collect();

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
        assert_eq!(cfg.rules.len(), 2);
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
}
