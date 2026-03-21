mod cache;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Mutex;

use anyhow::{Result, bail};
use clap::Parser;
use ignore::WalkBuilder;
use rayon::prelude::*;
use tracing::debug;

use gale_config::{ConfigResolver, GaleConfig};
use gale_css_parser::detect_syntax;
use gale_diagnostics::{LintResult, Severity, apply_fixes};
use gale_formatter::create_formatter;
use gale_linter::{LintRunner, RuleRegistry};

use crate::cache::{LintCache, compute_config_hash, compute_hash, resolve_cache_path};

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "gale",
    version,
    about = "An extremely fast CSS linter, written in Rust"
)]
pub struct Cli {
    /// Files or glob patterns to lint
    #[arg(required_unless_present_any = ["stdin", "init", "lsp", "print_config"])]
    files: Vec<String>,

    /// Resolve and print the effective config for FILE as JSON, then exit
    #[arg(long, value_name = "FILE")]
    print_config: Option<PathBuf>,

    /// Start the LSP server (for editor integration)
    #[arg(long)]
    lsp: bool,

    /// Generate a starter gale.json config file in the current directory
    #[arg(long)]
    init: bool,

    /// Read source from stdin instead of files
    #[arg(long)]
    stdin: bool,

    /// Virtual filename for stdin input (used for syntax detection and diagnostics)
    #[arg(long, default_value = "stdin.css")]
    stdin_filename: String,

    /// Config file path
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Output format
    #[arg(short = 'f', long, default_value = "text")]
    formatter: String,

    /// Maximum number of warnings before erroring
    #[arg(long)]
    max_warnings: Option<usize>,

    /// Automatically fix problems
    #[arg(long)]
    fix: bool,

    /// Only report errors
    #[arg(short, long)]
    quiet: bool,

    /// Path to a custom ignore file (uses gitignore syntax)
    #[arg(long, value_name = "FILE")]
    ignore_path: Option<PathBuf>,

    /// Disable all ignore file processing (gitignore, .galeignore, custom)
    #[arg(long)]
    no_ignore: bool,

    /// Enable file-based caching to skip unchanged files
    #[arg(long)]
    cache: bool,

    /// Override the cache file location (default: .gale_cache in the current directory)
    #[arg(long, value_name = "PATH")]
    cache_location: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Supported file extensions
// ---------------------------------------------------------------------------

const CSS_EXTENSIONS: &[&str] = &["css", "scss", "less", "sass"];

fn is_css_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| CSS_EXTENSIONS.contains(&ext))
}

// ---------------------------------------------------------------------------
// File discovery
// ---------------------------------------------------------------------------

struct DiscoverOptions<'a> {
    no_ignore: bool,
    ignore_path: Option<&'a Path>,
    ignore_patterns: &'a [String],
}

/// Build an `ignore::gitignore::Gitignore` matcher from `.stylelintignore`,
/// `.galeignore`, and an optional user-supplied ignore file in the cwd.
///
/// This is used to filter explicitly-passed files, matching Stylelint's
/// behaviour of always respecting `.stylelintignore` even for explicit paths.
fn build_explicit_ignore_matcher(
    user_ignore: Option<&Path>,
) -> Option<ignore::gitignore::Gitignore> {
    let cwd = std::env::current_dir().ok()?;
    let mut builder = ignore::gitignore::GitignoreBuilder::new(&cwd);

    // Load .stylelintignore from cwd (if it exists).
    let stylelintignore = cwd.join(".stylelintignore");
    if stylelintignore.is_file() {
        builder.add(&stylelintignore);
    }

    // Load .galeignore from cwd (if it exists).
    let galeignore = cwd.join(".galeignore");
    if galeignore.is_file() {
        builder.add(&galeignore);
    }

    // Load user-supplied ignore file.
    if let Some(path) = user_ignore {
        if path.is_file() {
            builder.add(path);
        }
    }

    builder.build().ok()
}

/// Returns `true` if the string contains glob meta-characters (`*`, `?`, `{`, `[`).
fn is_glob_pattern(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('{') || s.contains('[')
}

fn discover_files(paths: &[String], opts: &DiscoverOptions<'_>) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();

    // Build a gitignore matcher for .stylelintignore / .galeignore so we can
    // filter explicitly-passed files the same way Stylelint does.
    let explicit_ignore = if opts.no_ignore {
        None
    } else {
        build_explicit_ignore_matcher(opts.ignore_path)
    };

    // Build glob matchers from config ignore patterns (ignoreFiles / ignorePatterns).
    // These are applied to both explicit files and walked directories, matching
    // Stylelint's behaviour where `ignoreFiles` always excludes matching paths.
    let cwd = std::env::current_dir().unwrap_or_default();
    let config_ignore_matchers: Vec<globset::GlobMatcher> = opts
        .ignore_patterns
        .iter()
        .filter_map(|pat| {
            // Stylelint ignoreFiles patterns are relative to config location (usually
            // project root). Strip leading "./" for matching and treat as globs.
            let clean = pat.strip_prefix("./").unwrap_or(pat);
            globset::Glob::new(clean)
                .ok()
                .map(|g| g.compile_matcher())
        })
        .collect();

    for pattern in paths {
        // If the argument contains glob meta-characters, use the `ignore`
        // crate's `WalkBuilder` (same as for directory walks) so that
        // `.gitignore`, `.galeignore`, `.stylelintignore`, and default
        // `node_modules` exclusion are all respected.  The glob pattern is
        // compiled via `globset` and used as a post-filter on the walked
        // paths.
        if is_glob_pattern(pattern) {
            // Compile the glob pattern for matching against walked paths.
            // Use `literal_separator(true)` so that `*` does not match `/`,
            // preserving standard glob semantics where `*.css` only matches
            // files in the current directory while `**/*.css` matches
            // recursively.
            let glob_matcher = match globset::GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
            {
                Ok(g) => g.compile_matcher(),
                Err(err) => {
                    eprintln!("Warning: invalid glob pattern '{pattern}': {err}");
                    continue;
                }
            };

            // Determine the walk root: use the longest literal prefix of the
            // pattern (before the first meta-character).  Fall back to "."
            // when the pattern starts with a meta-character (e.g. `**/*.css`).
            let walk_root = {
                let first_meta = pattern
                    .find(|c: char| c == '*' || c == '?' || c == '{' || c == '[')
                    .unwrap_or(pattern.len());
                let prefix = &pattern[..first_meta];
                let root = Path::new(prefix)
                    .parent()
                    .unwrap_or_else(|| Path::new("."));
                if root.as_os_str().is_empty() {
                    PathBuf::from(".")
                } else {
                    root.to_path_buf()
                }
            };

            let mut builder = WalkBuilder::new(&walk_root);
            builder.hidden(false);

            if opts.no_ignore {
                builder.git_ignore(false);
                builder.git_global(false);
                builder.git_exclude(false);
            } else {
                builder.git_ignore(true);
                builder.add_custom_ignore_filename(".galeignore");
                builder.add_custom_ignore_filename(".stylelintignore");

                if let Some(ignore_file) = opts.ignore_path {
                    if ignore_file.exists() {
                        if let Some(err) = builder.add_ignore(ignore_file) {
                            eprintln!(
                                "Warning: failed to load ignore file {}: {err}",
                                ignore_file.display()
                            );
                        }
                    }
                }

                if !opts.ignore_patterns.is_empty() {
                    let mut overrides =
                        ignore::overrides::OverrideBuilder::new(&walk_root);
                    for pat in opts.ignore_patterns {
                        if let Err(err) = overrides.add(&format!("!{pat}")) {
                            eprintln!("Warning: invalid ignore pattern '{pat}': {err}");
                        }
                    }
                    if let Ok(built) = overrides.build() {
                        builder.overrides(built);
                    }
                }
            }

            for entry in builder.build().flatten() {
                let entry_path = entry.path();
                if entry_path.is_file() && is_css_file(entry_path) {
                    // Check the glob pattern against the path.
                    // Check the glob pattern against the path.
                    if !glob_matcher.is_match(entry_path) {
                        // Also try matching the relative path from cwd.
                        let rel = entry_path
                            .strip_prefix(&cwd)
                            .unwrap_or(entry_path);
                        if !glob_matcher.is_match(rel) {
                            continue;
                        }
                    }

                    // Respect .stylelintignore / .galeignore for explicitly
                    // matched files (belt-and-suspenders with WalkBuilder).
                    if let Some(ref gi) = explicit_ignore {
                        if gi
                            .matched_path_or_any_parents(entry_path, false)
                            .is_ignore()
                        {
                            continue;
                        }
                    }

                    // Check config ignore patterns.
                    if !config_ignore_matchers.is_empty() {
                        let abs = entry_path
                            .canonicalize()
                            .unwrap_or_else(|_| entry_path.to_path_buf());
                        let rel = abs.strip_prefix(&cwd).unwrap_or(&abs);
                        if config_ignore_matchers.iter().any(|m| m.is_match(rel)) {
                            continue;
                        }
                    }

                    files.push(entry_path.to_path_buf());
                }
            }
            continue;
        }

        let path = Path::new(pattern);

        if path.is_file() {
            if is_css_file(path) {
                // Respect .stylelintignore / .galeignore even for explicit files
                // (matching Stylelint's behaviour).
                if let Some(ref gi) = explicit_ignore {
                    // matched_path_or_any_parents checks the path and all its
                    // ancestor components, so `packages/theme/src/prebuilt/x.css`
                    // will match a pattern `packages/theme/src/prebuilt`.
                    if gi.matched_path_or_any_parents(path, false).is_ignore() {
                        continue;
                    }
                }

                // Check config ignore patterns (ignoreFiles / ignorePatterns).
                if !config_ignore_matchers.is_empty() {
                    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
                    let rel = abs.strip_prefix(&cwd).unwrap_or(&abs);
                    if config_ignore_matchers.iter().any(|m| m.is_match(rel)) {
                        continue;
                    }
                }

                files.push(path.to_path_buf());
            }
            continue;
        }

        let mut builder = WalkBuilder::new(path);

        // Show hidden CSS files (e.g. .hidden.css) but respect ignore files.
        builder.hidden(false);

        if opts.no_ignore {
            // Disable all ignore-file processing.
            builder.git_ignore(false);
            builder.git_global(false);
            builder.git_exclude(false);
        } else {
            // Respect .gitignore (enabled by default, but be explicit).
            builder.git_ignore(true);

            // Automatically respect .galeignore files found in traversed dirs.
            builder.add_custom_ignore_filename(".galeignore");

            // Respect .stylelintignore files (Stylelint compatibility).
            builder.add_custom_ignore_filename(".stylelintignore");

            // Load a user-supplied custom ignore file.
            if let Some(ignore_file) = opts.ignore_path {
                if ignore_file.exists() {
                    if let Some(err) = builder.add_ignore(ignore_file) {
                        eprintln!(
                            "Warning: failed to load ignore file {}: {err}",
                            ignore_file.display()
                        );
                    }
                } else {
                    eprintln!("Warning: ignore file not found: {}", ignore_file.display());
                }
            }

            // Apply ignore_patterns from config as glob overrides.
            if !opts.ignore_patterns.is_empty() {
                let mut overrides = ignore::overrides::OverrideBuilder::new(path);
                for pat in opts.ignore_patterns {
                    // Negate the pattern so matching files are excluded.
                    if let Err(err) = overrides.add(&format!("!{pat}")) {
                        eprintln!("Warning: invalid ignore pattern '{pat}': {err}");
                    }
                }
                if let Ok(built) = overrides.build() {
                    builder.overrides(built);
                }
            }
        }

        let walker = builder.build();

        for entry in walker.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file() && is_css_file(entry_path) {
                files.push(entry_path.to_path_buf());
            }
        }
    }

    files
}

// ---------------------------------------------------------------------------
// Config initialisation (--init)
// ---------------------------------------------------------------------------

/// Config file names that, if present, prevent `--init` from creating a new one.
const EXISTING_CONFIG_FILES: &[&str] = &["gale.json", "gale.toml", ".stylelintrc.json"];

fn generate_init_config() -> Result<()> {
    let cwd = std::env::current_dir()?;

    for name in EXISTING_CONFIG_FILES {
        let path = cwd.join(name);
        if path.exists() {
            bail!("Configuration file already exists: {}", path.display());
        }
    }

    let config_content = r#"{
  "extends": "gale:recommended",
  "rules": {}
}
"#;

    let config_path = cwd.join("gale.json");
    std::fs::write(&config_path, config_content)?;
    println!("Created gale.json with recommended configuration.");
    Ok(())
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    // Initialise tracing (controlled via GALE_LOG env var).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("GALE_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    // Handle --lsp: start the LSP server and exit early.
    if cli.lsp {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(gale_lsp::run_server());
        return Ok(());
    }

    // Handle --init: generate a starter config file and exit early.
    if cli.init {
        return generate_init_config();
    }

    // Resolve configuration.
    //
    // `has_config_file` tracks whether the user has *any* config file (even if
    // it resolves to zero rules).  When true we respect whatever the config
    // says — including "no rules".  The "enable all rules" fallback only kicks
    // in when there is genuinely no config file anywhere.
    //
    // When `--config` is NOT specified we use a `ConfigResolver` for per-file
    // config lookup (matching Stylelint's cosmiconfig behaviour where each file
    // is linted with the closest config in the directory hierarchy).
    let use_per_file_config = cli.config.is_none();
    let resolver = Mutex::new(ConfigResolver::new());

    let (config, has_config_file) = if let Some(ref cfg_path) = cli.config {
        debug!("Using config file: {}", cfg_path.display());
        let cfg = gale_config::load_config(cfg_path).unwrap_or_else(|err| {
            eprintln!("Warning: failed to load config: {err}");
            GaleConfig::default()
        });
        (cfg, true)
    } else {
        let cwd = std::env::current_dir().unwrap_or_default();
        match gale_config::resolve_config(&cwd) {
            Some(cfg) => (cfg, true),
            None => (GaleConfig::default(), false),
        }
    };

    // Build rule registry and determine enabled rules.
    let registry = RuleRegistry::default();
    let enabled_rules: Vec<String> = if config.rules.is_empty() && !has_config_file {
        // No config file found at all — enable all registered rules as a
        // sensible default so `gale .` works out-of-the-box.
        registry
            .all()
            .iter()
            .map(|r| r.name().to_string())
            .collect()
    } else {
        config
            .rules
            .iter()
            .filter(|(_, cfg)| {
                cfg.severity
                    .as_ref()
                    .map(|s| !matches!(s, gale_config::Severity::Off))
                    .unwrap_or(true)
            })
            .filter(|(name, _)| registry.get(name).is_some())
            .map(|(name, _)| name.clone())
            .collect()
    };

    // Handle --print-config: print resolved config as JSON and exit.
    if let Some(ref file) = cli.print_config {
        let file_str = file.display().to_string();
        // Use per-file config resolution when --config is not specified.
        let file_config;
        let effective_config = if use_per_file_config {
            let abs_path = std::env::current_dir()
                .unwrap_or_default()
                .join(file);
            file_config = gale_config::resolve_config_for_file(&abs_path)
                .unwrap_or_else(|| config.clone());
            &file_config
        } else {
            &config
        };
        let effective_rules = effective_config.rules_for_file(&file_str);
        let mut rules_json = serde_json::Map::new();
        for (name, rc) in &effective_rules {
            let severity = rc
                .severity
                .as_ref()
                .map(|s| format!("{s:?}").to_lowercase())
                .unwrap_or_else(|| "error".to_string());
            if let Some(ref opts) = rc.options {
                // Show [severity, options] for rules with options.
                rules_json.insert(name.clone(), serde_json::json!([severity, opts]));
            } else {
                rules_json.insert(name.clone(), serde_json::Value::String(severity));
            }
        }
        let output = serde_json::json!({
            "file": file_str,
            "rules": rules_json,
            "ignorePatterns": effective_config.ignore_patterns,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    // Set up caching if --cache is enabled.
    let cache_path = resolve_cache_path(cli.cache_location.as_deref());
    let config_hash = compute_config_hash(&config.rules);
    let mut lint_cache = if cli.cache {
        debug!("Loading cache from {}", cache_path.display());
        LintCache::load(&cache_path)
    } else {
        LintCache::default()
    };

    // Extract per-rule options from the config.
    let rule_options: std::collections::HashMap<String, serde_json::Value> = config
        .rules
        .iter()
        .filter_map(|(name, cfg)| {
            cfg.options
                .as_ref()
                .map(|opts| (name.clone(), opts.clone()))
        })
        .collect();

    // Extract per-rule severity overrides from the config.
    let rule_severities: std::collections::HashMap<String, gale_diagnostics::Severity> = config
        .rules
        .iter()
        .filter_map(|(name, cfg)| {
            cfg.severity.as_ref().and_then(|s| match s {
                gale_config::Severity::Error => {
                    Some((name.clone(), gale_diagnostics::Severity::Error))
                }
                gale_config::Severity::Warning => {
                    Some((name.clone(), gale_diagnostics::Severity::Warning))
                }
                gale_config::Severity::Off => None,
            })
        })
        .collect();

    let runner = LintRunner::with_options_and_severities(
        registry,
        enabled_rules.clone(),
        rule_options,
        rule_severities,
    );
    let has_overrides = config.has_overrides();

    /// Lint a single file using a fully resolved config.
    ///
    /// Extracts enabled rules, options, and severities from the config (applying
    /// per-file overrides) and runs the linter.
    fn lint_file_with_resolved_config(
        runner: &LintRunner,
        source: &str,
        file_path: &str,
        syntax: gale_css_parser::Syntax,
        config: &GaleConfig,
    ) -> LintResult {
        let effective_rules = config.rules_for_file(file_path);
        let file_enabled: Vec<String> = effective_rules
            .iter()
            .filter(|(_, cfg)| {
                cfg.severity
                    .as_ref()
                    .map(|s| !matches!(s, gale_config::Severity::Off))
                    .unwrap_or(true)
            })
            .filter(|(name, _)| runner.has_rule(name))
            .map(|(name, _)| name.clone())
            .collect();
        let override_options: std::collections::HashMap<String, serde_json::Value> =
            effective_rules
                .iter()
                .filter_map(|(name, cfg)| {
                    cfg.options
                        .as_ref()
                        .map(|opts| (name.clone(), opts.clone()))
                })
                .collect();
        let override_severities: std::collections::HashMap<
            String,
            gale_diagnostics::Severity,
        > = effective_rules
            .iter()
            .filter_map(|(name, cfg)| {
                cfg.severity.as_ref().and_then(|s| match s {
                    gale_config::Severity::Error => {
                        Some((name.clone(), gale_diagnostics::Severity::Error))
                    }
                    gale_config::Severity::Warning => {
                        Some((name.clone(), gale_diagnostics::Severity::Warning))
                    }
                    gale_config::Severity::Off => None,
                })
            })
            .collect();
        runner.lint_source_with_rules(
            source,
            file_path,
            syntax,
            &file_enabled,
            &override_options,
            &override_severities,
        )
    }

    /// Lint a single file, computing the effective rules from overrides if needed.
    fn lint_file(
        runner: &LintRunner,
        source: &str,
        file_path: &str,
        syntax: gale_css_parser::Syntax,
        config: &GaleConfig,
        has_overrides: bool,
    ) -> LintResult {
        if has_overrides {
            lint_file_with_resolved_config(runner, source, file_path, syntax, config)
        } else {
            runner.lint_source(source, file_path, syntax)
        }
    }

    // Lint: either from stdin or from discovered files.
    let mut results: Vec<LintResult> = if cli.stdin {
        let mut source = String::new();
        std::io::stdin().read_to_string(&mut source)?;

        let file_path = &cli.stdin_filename;
        let syntax = detect_syntax(file_path);
        let result = lint_file(&runner, &source, file_path, syntax, &config, has_overrides);
        vec![result]
    } else {
        // Discover files.
        let discover_opts = DiscoverOptions {
            no_ignore: cli.no_ignore,
            ignore_path: cli.ignore_path.as_deref(),
            ignore_patterns: &config.ignore_patterns,
        };
        let files = discover_files(&cli.files, &discover_opts);
        debug!("Discovered {} CSS file(s)", files.len());

        if files.is_empty() {
            eprintln!("No CSS files found.");
            return Ok(());
        }

        if cli.cache {
            // With caching: read files, check cache, skip clean ones.
            let cache_mutex = Mutex::new(&mut lint_cache);
            let results: Vec<LintResult> = files
                .par_iter()
                .filter_map(|file| {
                    let source = std::fs::read_to_string(file).ok()?;
                    let file_path = file.display().to_string();
                    let content_hash = compute_hash(&source, config_hash);

                    // Check cache: skip only files that were clean (0 diagnostics).
                    {
                        let cache = cache_mutex.lock().unwrap_or_else(|e| e.into_inner());
                        if cache.is_clean(&file_path, content_hash) {
                            debug!("Cache hit (clean): {file_path}");
                            return None;
                        }
                    }

                    let syntax = detect_syntax(&file_path);
                    let result = if use_per_file_config {
                        let abs = std::env::current_dir().unwrap_or_default().join(file);
                        let mut res_guard = resolver.lock().unwrap_or_else(|e| e.into_inner());
                        let file_cfg = res_guard.resolve_for_file(&abs)
                            .cloned()
                            .unwrap_or_else(|| config.clone());
                        drop(res_guard);
                        lint_file_with_resolved_config(&runner, &source, &file_path, syntax, &file_cfg)
                    } else {
                        lint_file(&runner, &source, &file_path, syntax, &config, has_overrides)
                    };

                    // Update cache with the new result.
                    {
                        let mut cache = cache_mutex.lock().unwrap_or_else(|e| e.into_inner());
                        cache.record(file_path, content_hash, result.diagnostics.len());
                    }

                    Some(result)
                })
                .collect();
            results
        } else {
            // Without caching: lint each file in parallel.
            files
                .par_iter()
                .filter_map(|file| {
                    let source = std::fs::read_to_string(file).ok()?;
                    let file_path = file.display().to_string();
                    let syntax = detect_syntax(&file_path);
                    if use_per_file_config {
                        let abs = std::env::current_dir().unwrap_or_default().join(file);
                        let mut res_guard = resolver.lock().unwrap_or_else(|e| e.into_inner());
                        let file_cfg = res_guard.resolve_for_file(&abs)
                            .cloned()
                            .unwrap_or_else(|| config.clone());
                        drop(res_guard);
                        Some(lint_file_with_resolved_config(
                            &runner, &source, &file_path, syntax, &file_cfg,
                        ))
                    } else {
                        Some(lint_file(
                            &runner,
                            &source,
                            &file_path,
                            syntax,
                            &config,
                            has_overrides,
                        ))
                    }
                })
                .collect()
        }
    };

    // Apply fixes when --fix is set.
    if cli.fix {
        let mut total_fixed = 0usize;
        for result in &mut results {
            let (fixed_source, count) = apply_fixes(&result.source, &result.diagnostics);
            if count > 0 {
                if cli.stdin {
                    // For stdin + fix, output the fixed source to stdout.
                    print!("{fixed_source}");
                    total_fixed += count;
                    // Re-lint the fixed source to get remaining diagnostics.
                    let syntax = detect_syntax(&result.file_path);
                    *result = lint_file(
                        &runner,
                        &fixed_source,
                        &result.file_path,
                        syntax,
                        &config,
                        has_overrides,
                    );
                } else if let Err(err) = std::fs::write(&result.file_path, &fixed_source) {
                    eprintln!("Error writing {}: {err}", result.file_path);
                } else {
                    total_fixed += count;
                    // Re-lint the fixed source to get remaining diagnostics.
                    let syntax = detect_syntax(&result.file_path);
                    *result = lint_file(
                        &runner,
                        &fixed_source,
                        &result.file_path,
                        syntax,
                        &config,
                        has_overrides,
                    );
                }
            }
        }
        if total_fixed > 0 {
            eprintln!("Fixed {total_fixed} problem(s).");
        }
    }

    // Filter to errors-only when --quiet is set.
    if cli.quiet {
        for result in &mut results {
            result.diagnostics.retain(|d| d.severity == Severity::Error);
        }
    }

    // Save cache if --cache is enabled.
    if cli.cache {
        debug!("Saving cache to {}", cache_path.display());
        lint_cache.save(&cache_path);
    }

    // Format & print.
    let t_fmt = std::time::Instant::now();
    let formatter = create_formatter(&cli.formatter);
    let output = gale_formatter::Formatter::format(&*formatter, &results);
    if std::env::var("GALE_DEBUG_PERF").as_deref() == Ok("1") {
        eprintln!("[perf] format: {:.3}s", t_fmt.elapsed().as_secs_f64());
    }
    if !output.is_empty() {
        print!("{output}");
    }

    // Summarise counts.
    let total_errors: usize = results
        .iter()
        .map(|r| r.count_by_severity(Severity::Error))
        .sum();
    let total_warnings: usize = results
        .iter()
        .map(|r| r.count_by_severity(Severity::Warning))
        .sum();

    // Check --max-warnings threshold.
    if let Some(max) = cli.max_warnings
        && total_warnings > max
    {
        eprintln!("Found {total_warnings} warning(s) (max allowed: {max})");
        process::exit(1);
    }

    // Exit with code 1 if there were any errors.
    if total_errors > 0 {
        process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create a temporary directory tree with CSS files for glob tests.
    fn create_test_tree(base: &Path) {
        let sub = base.join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(base.join("a.css"), "a {}").unwrap();
        fs::write(base.join("b.scss"), "$x: 1;").unwrap();
        fs::write(base.join("c.txt"), "not css").unwrap();
        fs::write(sub.join("d.css"), "d {}").unwrap();
        fs::write(sub.join("e.less"), ".e {}").unwrap();
    }

    fn default_opts() -> DiscoverOptions<'static> {
        DiscoverOptions {
            no_ignore: true,
            ignore_path: None,
            ignore_patterns: &[],
        }
    }

    #[test]
    fn test_is_glob_pattern() {
        assert!(is_glob_pattern("**/*.css"));
        assert!(is_glob_pattern("src/*.scss"));
        assert!(is_glob_pattern("src/{a,b}.css"));
        assert!(is_glob_pattern("src/[ab].css"));
        assert!(is_glob_pattern("src/??.css"));
        assert!(!is_glob_pattern("src/file.css"));
        assert!(!is_glob_pattern("src/dir"));
        assert!(!is_glob_pattern("plain-path"));
    }

    #[test]
    fn test_glob_star_css() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_tree(tmp.path());

        let pattern = format!("{}/*.css", tmp.path().display());
        let files = discover_files(&[pattern], &default_opts());

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("a.css"));
    }

    #[test]
    fn test_glob_recursive_double_star() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_tree(tmp.path());

        let pattern = format!("{}/**/*.css", tmp.path().display());
        let files = discover_files(&[pattern], &default_opts());

        // Should find a.css and sub/d.css
        assert_eq!(files.len(), 2);
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"a.css".to_string()));
        assert!(names.contains(&"d.css".to_string()));
    }

    #[test]
    fn test_glob_scss_only() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_tree(tmp.path());

        let pattern = format!("{}/**/*.scss", tmp.path().display());
        let files = discover_files(&[pattern], &default_opts());

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("b.scss"));
    }

    #[test]
    fn test_glob_no_match_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_tree(tmp.path());

        let pattern = format!("{}/**/*.sass", tmp.path().display());
        let files = discover_files(&[pattern], &default_opts());

        assert!(files.is_empty());
    }

    #[test]
    fn test_glob_mixed_with_directory_arg() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_tree(tmp.path());

        // Mix a glob pattern with a plain directory argument.
        let glob_pat = format!("{}/*.scss", tmp.path().display());
        let dir_arg = tmp.path().join("sub").display().to_string();
        let files = discover_files(&[glob_pat, dir_arg], &default_opts());

        // Glob should find b.scss; directory walk should find d.css + e.less.
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_non_glob_file_still_works() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_tree(tmp.path());

        let file_arg = tmp.path().join("a.css").display().to_string();
        let files = discover_files(&[file_arg], &default_opts());

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("a.css"));
    }

    #[test]
    fn test_non_glob_directory_still_works() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_tree(tmp.path());

        let dir_arg = tmp.path().display().to_string();
        let files = discover_files(&[dir_arg], &default_opts());

        // Should find all CSS files recursively: a.css, b.scss, sub/d.css, sub/e.less
        assert_eq!(files.len(), 4);
    }
}
