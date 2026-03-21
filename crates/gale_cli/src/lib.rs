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

use gale_config::GaleConfig;
use gale_css_parser::detect_syntax;
use gale_diagnostics::{LintResult, Severity, apply_fixes};
use gale_formatter::create_formatter;
use gale_linter::{LintRunner, RuleRegistry};

use crate::cache::{LintCache, compute_config_hash, compute_hash, resolve_cache_path};

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "gale", about = "An extremely fast CSS linter, written in Rust")]
pub struct Cli {
    /// Files or glob patterns to lint
    #[arg(required_unless_present_any = ["stdin", "init", "lsp"])]
    files: Vec<String>,

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

fn discover_files(paths: &[String], opts: &DiscoverOptions<'_>) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();

    for pattern in paths {
        let path = Path::new(pattern);

        if path.is_file() {
            if is_css_file(path) {
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
                    eprintln!(
                        "Warning: ignore file not found: {}",
                        ignore_file.display()
                    );
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
            bail!(
                "Configuration file already exists: {}",
                path.display()
            );
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
    let config = if let Some(ref cfg_path) = cli.config {
        debug!("Using config file: {}", cfg_path.display());
        gale_config::load_config(cfg_path).unwrap_or_else(|err| {
            eprintln!("Warning: failed to load config: {err}");
            GaleConfig::default()
        })
    } else {
        let cwd = std::env::current_dir().unwrap_or_default();
        gale_config::resolve_config(&cwd)
    };

    // Build rule registry and determine enabled rules.
    let registry = RuleRegistry::default();
    let enabled_rules: Vec<String> = if config.rules.is_empty() {
        // If no rules configured, enable all registered rules.
        registry.all().iter().map(|r| r.name().to_string()).collect()
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
            .map(|(name, _)| name.clone())
            .collect()
    };

    // Set up caching if --cache is enabled.
    let cache_path = resolve_cache_path(cli.cache_location.as_deref());
    let config_hash = compute_config_hash(&enabled_rules);
    let mut lint_cache = if cli.cache {
        debug!("Loading cache from {}", cache_path.display());
        LintCache::load(&cache_path)
    } else {
        LintCache::default()
    };

    let runner = LintRunner::new(registry, enabled_rules);

    // Lint: either from stdin or from discovered files.
    let mut results: Vec<LintResult> = if cli.stdin {
        let mut source = String::new();
        std::io::stdin()
            .read_to_string(&mut source)
            .expect("Failed to read from stdin");

        let file_path = &cli.stdin_filename;
        let syntax = detect_syntax(file_path);
        let result = runner.lint_source(&source, file_path, syntax);
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
                        let cache = cache_mutex.lock().unwrap();
                        if cache.is_clean(&file_path, content_hash) {
                            debug!("Cache hit (clean): {file_path}");
                            return None;
                        }
                    }

                    let syntax = detect_syntax(&file_path);
                    let result = runner.lint_source(&source, &file_path, syntax);

                    // Update cache with the new result.
                    {
                        let mut cache = cache_mutex.lock().unwrap();
                        cache.record(
                            file_path,
                            content_hash,
                            result.diagnostics.len(),
                        );
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
                    Some(runner.lint_source(&source, &file_path, syntax))
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
                    *result = runner.lint_source(&fixed_source, &result.file_path, syntax);
                } else if let Err(err) = std::fs::write(&result.file_path, &fixed_source) {
                    eprintln!("Error writing {}: {err}", result.file_path);
                } else {
                    total_fixed += count;
                    // Re-lint the fixed source to get remaining diagnostics.
                    let syntax = detect_syntax(&result.file_path);
                    *result = runner.lint_source(&fixed_source, &result.file_path, syntax);
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
            result
                .diagnostics
                .retain(|d| d.severity == Severity::Error);
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
