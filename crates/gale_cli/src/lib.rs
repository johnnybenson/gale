use std::path::{Path, PathBuf};
use std::process;

use anyhow::Result;
use clap::Parser;
use ignore::WalkBuilder;
use rayon::prelude::*;
use tracing::debug;

use gale_config::GaleConfig;
use gale_css_parser::detect_syntax;
use gale_diagnostics::{LintResult, Severity};
use gale_formatter::create_formatter;
use gale_linter::{LintRunner, RuleRegistry};

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "gale", about = "An extremely fast CSS linter, written in Rust")]
pub struct Cli {
    /// Files or glob patterns to lint
    #[arg(required = true)]
    files: Vec<String>,

    /// Config file path
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Output format
    #[arg(short = 'f', long, default_value = "text")]
    formatter: String,

    /// Maximum number of warnings before erroring
    #[arg(long)]
    max_warnings: Option<usize>,

    /// Only report errors
    #[arg(short, long)]
    quiet: bool,
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

fn discover_files(paths: &[String]) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();

    for pattern in paths {
        let path = Path::new(pattern);

        if path.is_file() {
            if is_css_file(path) {
                files.push(path.to_path_buf());
            }
            continue;
        }

        let walker = WalkBuilder::new(path).hidden(false).build();

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

    // Discover files.
    let files = discover_files(&cli.files);
    debug!("Discovered {} CSS file(s)", files.len());

    if files.is_empty() {
        eprintln!("No CSS files found.");
        return Ok(());
    }

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

    let runner = LintRunner::new(registry, enabled_rules);

    // Lint each file in parallel.
    let mut results: Vec<LintResult> = files
        .par_iter()
        .filter_map(|file| {
            let source = std::fs::read_to_string(file).ok()?;
            let file_path = file.display().to_string();
            let syntax = detect_syntax(&file_path);
            Some(runner.lint_source(&source, &file_path, syntax))
        })
        .collect();

    // Filter to errors-only when --quiet is set.
    if cli.quiet {
        for result in &mut results {
            result
                .diagnostics
                .retain(|d| d.severity == Severity::Error);
        }
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
    if let Some(max) = cli.max_warnings {
        if total_warnings > max {
            eprintln!("Found {total_warnings} warning(s) (max allowed: {max})");
            process::exit(1);
        }
    }

    // Exit with code 1 if there were any errors.
    if total_errors > 0 {
        process::exit(1);
    }

    Ok(())
}
