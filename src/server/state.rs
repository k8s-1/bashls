use std::collections::HashMap;

use crossbeam_channel::Sender;
use lsp_server::{Connection, Message, Notification};
use lsp_types::notification::Notification as _;
use lsp_types::{
    CodeAction, CompletionOptions, Diagnostic, PublishDiagnosticsParams, RenameOptions,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
    notification::PublishDiagnostics,
};
use serde_json::Value;

use crate::analyser::Analyser;
use crate::config::Config;
use crate::executables::Executables;
use crate::shellcheck::Linter;
use crate::shfmt::Formatter;
use crate::util::fs::uri_to_path;

pub struct LintResult {
    pub uri: String,
    pub version: Option<i32>,
    pub diagnostics: Vec<Diagnostic>,
    pub code_actions: HashMap<String, CodeAction>,
}

pub struct DocumentState {
    pub content: String,
    pub version: i32,
}

pub struct Server {
    pub analyser: Analyser,
    pub config: Config,
    pub executables: Executables,
    pub linter: Option<Linter>,
    pub formatter: Option<Formatter>,
    pub workspace_folder: Option<String>,
    pub documents: HashMap<String, DocumentState>,
    pub code_actions: HashMap<String, HashMap<String, CodeAction>>,
    pub initialized: bool,
    pub current_document: Option<String>,
    pub client_capabilities: lsp_types::ClientCapabilities,
    pub pending_config_request_id: Option<lsp_server::RequestId>,
    pub next_request_id: i32,
}

impl Server {
    pub(crate) fn analyze_and_lint(
        &mut self,
        uri: &str,
        content: &str,
        connection: &Connection,
        lint_tx: &Sender<LintResult>,
    ) {
        let ts_diagnostics = self.analyser.analyze(uri, content);
        let version = self.documents.get(uri).map(|d| d.version);

        send_diagnostics(&connection.sender, uri, version, ts_diagnostics.clone());

        if let Some(linter) = self.linter.clone() {
            let source_paths = self
                .workspace_folder
                .as_ref()
                .map(|w| vec![uri_to_path(w).to_string_lossy().into_owned()])
                .unwrap_or_default();
            let args = self.config.shellcheck_arguments.clone();
            let uri = uri.to_string();
            let content = content.to_string();
            let tx = lint_tx.clone();
            std::thread::spawn(move || {
                let result = linter.lint(&uri, &content, &source_paths, &args);
                let mut diagnostics = ts_diagnostics;
                diagnostics.extend(result.diagnostics);
                let _ = tx.send(LintResult {
                    uri,
                    version,
                    diagnostics,
                    code_actions: result.code_actions,
                });
            });
        }
    }

    pub(crate) fn apply_lint_result(&mut self, result: LintResult, sender: &Sender<Message>) {
        self.code_actions
            .insert(result.uri.clone(), result.code_actions);
        send_diagnostics(sender, &result.uri, result.version, result.diagnostics);
    }

    pub(crate) fn update_config(&mut self, value: &Value) {
        let bash_ide = value.get("bashIde").unwrap_or(value);
        if let Ok(cfg) = serde_json::from_value::<Config>(bash_ide.clone()) {
            let old_shellcheck = self.config.shellcheck_path.clone();
            let old_shfmt = self.config.shfmt.path.clone();
            self.config = cfg;

            if self.config.shellcheck_path.is_empty() {
                self.linter = None;
            } else if self.config.shellcheck_path != old_shellcheck || self.linter.is_none() {
                self.linter = Some(Linter::new(
                    self.config.shellcheck_path.clone(),
                    self.config.shellcheck_external_sources,
                ));
            }

            if self.config.shfmt.path.is_empty() {
                self.formatter = None;
            } else if self.config.shfmt.path != old_shfmt || self.formatter.is_none() {
                self.formatter = Some(Formatter::new(self.config.shfmt.path.clone()));
            }

            self.analyser
                .set_enable_source_error_diagnostics(self.config.enable_source_error_diagnostics);
            self.analyser
                .set_include_all_workspace_symbols(self.config.include_all_workspace_symbols);
        }
    }
}

fn send_diagnostics(
    sender: &Sender<Message>,
    uri: &str,
    version: Option<i32>,
    diagnostics: Vec<Diagnostic>,
) {
    let params = PublishDiagnosticsParams {
        uri: uri.parse().unwrap(),
        version,
        diagnostics,
    };
    let notif = Notification::new(PublishDiagnostics::METHOD.to_string(), params);
    let _ = sender.send(Message::Notification(notif));
}

pub(super) fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(true),
            trigger_characters: Some(vec!["$".to_string(), "{".to_string(), "-".to_string()]),
            ..Default::default()
        }),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        document_highlight_provider: Some(lsp_types::OneOf::Left(true)),
        definition_provider: Some(lsp_types::OneOf::Left(true)),
        document_symbol_provider: Some(lsp_types::OneOf::Left(true)),
        workspace_symbol_provider: Some(lsp_types::OneOf::Left(true)),
        references_provider: Some(lsp_types::OneOf::Left(true)),
        code_action_provider: Some(lsp_types::CodeActionProviderCapability::Options(
            lsp_types::CodeActionOptions {
                code_action_kinds: Some(vec![lsp_types::CodeActionKind::QUICKFIX]),
                resolve_provider: Some(false),
                work_done_progress_options: Default::default(),
            },
        )),
        rename_provider: Some(lsp_types::OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        document_formatting_provider: Some(lsp_types::OneOf::Left(true)),
        ..Default::default()
    }
}
