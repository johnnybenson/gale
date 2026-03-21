use std::sync::RwLock;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::debug;

use gale_config::GaleConfig;
use gale_css_parser::detect_syntax;
use gale_diagnostics::{Severity, SourceLineIndex};
use gale_linter::{LintRunner, RuleRegistry};

// ---------------------------------------------------------------------------
// Server state
// ---------------------------------------------------------------------------

pub struct GaleLspServer {
    client: Client,
    runner: RwLock<Option<LintRunner>>,
}

impl GaleLspServer {
    fn new(client: Client) -> Self {
        Self {
            client,
            runner: RwLock::new(None),
        }
    }

    /// Build `LintRunner` from the resolved config.
    fn build_runner(config: &GaleConfig) -> LintRunner {
        let registry = RuleRegistry::default();
        let enabled_rules: Vec<String> = if config.rules.is_empty() {
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
                .map(|(name, _)| name.clone())
                .collect()
        };
        LintRunner::new(registry, enabled_rules)
    }

    /// Lint source text and convert to LSP diagnostics (sync part).
    fn lint_to_diagnostics(
        &self,
        uri: &Url,
        source: &str,
    ) -> Vec<tower_lsp::lsp_types::Diagnostic> {
        let runner_guard = self.runner.read().unwrap();
        let Some(runner) = runner_guard.as_ref() else {
            return Vec::new();
        };

        let file_path = uri
            .to_file_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| uri.to_string());

        let syntax = detect_syntax(&file_path);
        let result = runner.lint_source(source, &file_path, syntax);

        let line_index = SourceLineIndex::build(source);

        result
            .diagnostics
            .iter()
            .map(|d| {
                let (start_line, start_col) = line_index.offset_to_location(d.span.offset);
                let (end_line, end_col) = line_index.offset_to_location(d.span.end());

                // SourceLineIndex returns 1-indexed; LSP uses 0-indexed.
                let range = Range {
                    start: Position {
                        line: start_line.saturating_sub(1) as u32,
                        character: start_col.saturating_sub(1) as u32,
                    },
                    end: Position {
                        line: end_line.saturating_sub(1) as u32,
                        character: end_col.saturating_sub(1) as u32,
                    },
                };

                let severity = match d.severity {
                    Severity::Error => Some(DiagnosticSeverity::ERROR),
                    Severity::Warning => Some(DiagnosticSeverity::WARNING),
                    Severity::Info => Some(DiagnosticSeverity::INFORMATION),
                    Severity::Hint => Some(DiagnosticSeverity::HINT),
                };

                tower_lsp::lsp_types::Diagnostic {
                    range,
                    severity,
                    code: Some(NumberOrString::String(d.rule_name.clone())),
                    source: Some("gale".to_string()),
                    message: d.message.clone(),
                    ..Default::default()
                }
            })
            .collect()
    }

    /// Lint source text and publish diagnostics to the client.
    async fn lint_and_publish(&self, uri: Url, source: &str) {
        let diagnostics = self.lint_to_diagnostics(&uri, source);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

// ---------------------------------------------------------------------------
// LanguageServer trait implementation
// ---------------------------------------------------------------------------

#[tower_lsp::async_trait]
impl LanguageServer for GaleLspServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Resolve config from workspace root if available.
        let config = if let Some(root_uri) = params.root_uri {
            if let Ok(root_path) = root_uri.to_file_path() {
                debug!("LSP workspace root: {}", root_path.display());
                gale_config::resolve_config(&root_path)
            } else {
                GaleConfig::default()
            }
        } else {
            let cwd = std::env::current_dir().unwrap_or_default();
            gale_config::resolve_config(&cwd)
        };

        // Build the lint runner once.
        let runner = Self::build_runner(&config);
        *self.runner.write().unwrap() = Some(runner);

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "gale-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        debug!("Gale LSP server initialized");
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let source = params.text_document.text;
        self.lint_and_publish(uri, &source).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // We use full sync, so the last change event contains the full text.
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            self.lint_and_publish(uri, &change.text).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        // If the save notification includes text, use it; otherwise read from disk.
        let source = if let Some(text) = params.text {
            text
        } else if let Ok(path) = uri.to_file_path() {
            match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(_) => return,
            }
        } else {
            return;
        };
        self.lint_and_publish(uri, &source).await;
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Start the Gale LSP server on stdin/stdout.
pub async fn run_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(GaleLspServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
