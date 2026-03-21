use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::debug;

/// On-disk representation of the lint cache.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct LintCache {
    /// Map from canonical file path to its cache entry.
    pub entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Hash of the file contents (combined with config hash).
    pub hash: u64,
    /// Number of diagnostics the linter reported for this file.
    pub diagnostics_count: usize,
}

impl LintCache {
    /// Load the cache from disk. Returns an empty cache on any error.
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_else(|err| {
                debug!("Failed to parse cache file: {err}");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Save the cache to disk.
    pub fn save(&self, path: &Path) {
        match serde_json::to_string(self) {
            Ok(data) => {
                if let Err(err) = std::fs::write(path, data) {
                    debug!("Failed to write cache file: {err}");
                }
            }
            Err(err) => {
                debug!("Failed to serialize cache: {err}");
            }
        }
    }

    /// Check whether a file can be skipped (hash matches and had 0 diagnostics).
    pub fn is_clean(&self, file_key: &str, content_hash: u64) -> bool {
        self.entries
            .get(file_key)
            .is_some_and(|e| e.hash == content_hash && e.diagnostics_count == 0)
    }

    /// Record a lint result in the cache.
    pub fn record(&mut self, file_key: String, content_hash: u64, diagnostics_count: usize) {
        self.entries.insert(
            file_key,
            CacheEntry {
                hash: content_hash,
                diagnostics_count,
            },
        );
    }
}

/// Compute a fast hash of the file contents combined with the config hash.
pub fn compute_hash(contents: &str, config_hash: u64) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    contents.hash(&mut hasher);
    config_hash.hash(&mut hasher);
    hasher.finish()
}

/// Compute a stable hash for the current linter configuration so that cache
/// entries are invalidated when the config changes.
pub fn compute_config_hash(enabled_rules: &[String]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    enabled_rules.hash(&mut hasher);
    hasher.finish()
}

/// Resolve the cache file path from CLI options.
pub fn resolve_cache_path(custom: Option<&Path>) -> PathBuf {
    if let Some(p) = custom {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".gale_cache")
    }
}
