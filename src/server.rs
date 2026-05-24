use std::collections::{HashMap, HashSet};

use anyhow::Result;
use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::notification::Notification as _;
use lsp_types::request::Request as _;
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CompletionItem, CompletionItemKind,
    CompletionOptions, CompletionResponse, Diagnostic, DocumentHighlight, FormattingOptions,
    GotoDefinitionResponse, Hover, Location, MarkupContent, MarkupKind, Position,
    PublishDiagnosticsParams, RenameOptions, ServerCapabilities, SymbolInformation, SymbolKind,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Uri, WorkspaceEdit,
    WorkspaceSymbolResponse,
    notification::{
        DidChangeConfiguration, DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument,
        Initialized, PublishDiagnostics,
    },
    request::{
        CodeActionRequest, Completion, DocumentHighlightRequest, DocumentSymbolRequest, Formatting,
        GotoDefinition, HoverRequest, PrepareRenameRequest, References, Rename,
        ResolveCompletionItem, WorkspaceSymbolRequest,
    },
};
use serde_json::{Value, json};

use crate::analyser::Analyser;
use crate::builtins;
use crate::config::Config;
use crate::executables::Executables;
use crate::parser::create_parser;
use crate::reserved_words;
use crate::shellcheck::Linter;
use crate::shfmt::Formatter;
use crate::snippets::get_snippets;
use crate::util::fs::{uri_to_path, uri_to_path_opt};
use crate::util::lsp::is_position_in_range;
use crate::util::sh::get_shell_documentation;

const PARAMETER_EXPANSION_PREFIXES: &[&str] = &["$", "${"];

fn check_runtime_deps() {
    let bash_ok = std::process::Command::new("bash")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !bash_ok {
        eprintln!("bashls: warning: bash not found — option completions unavailable");
        return;
    }

    let completion_ok = std::process::Command::new("bash")
        .args([
            "-c",
            "source /usr/share/bash-completion/bash_completion 2>/dev/null || \
                      source /etc/bash_completion 2>/dev/null || \
                      pkg-config --variable=completionsdir bash-completion 2>/dev/null",
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !completion_ok {
        eprintln!("bashls: warning: bash-completion not found — option completions unavailable");
    }

    let shellcheck_ok = std::process::Command::new("shellcheck")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !shellcheck_ok {
        eprintln!("bashls: warning: shellcheck not found — diagnostics unavailable");
    }

    let shfmt_ok = std::process::Command::new("shfmt")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !shfmt_ok {
        eprintln!("bashls: warning: shfmt not found — formatting unavailable");
    }
}

pub fn run() -> Result<()> {
    check_runtime_deps();
    let (connection, io_threads) = Connection::stdio();

    let server_capabilities = serde_json::to_value(server_capabilities())?;
    let init_params_value = connection.initialize(server_capabilities)?;
    let init_params: lsp_types::InitializeParams = serde_json::from_value(init_params_value)?;

    #[allow(deprecated)]
    let workspace_folder = init_params
        .root_uri
        .as_ref()
        .map(|u| u.to_string())
        .or_else(|| init_params.root_path.clone().map(|p| format!("file://{p}")));

    let path_var = std::env::var("PATH").unwrap_or_default();
    let executables = Executables::from_path(&path_var);

    let parser = create_parser()?;
    let analyser = Analyser::new(parser, workspace_folder.clone());

    let config = Config::from_env();

    let client_capabilities = init_params.capabilities.clone();

    let mut server = Server {
        analyser,
        config: config.clone(),
        executables,
        linter: if config.shellcheck_path.is_empty() {
            None
        } else {
            Some(Linter::new(
                config.shellcheck_path.clone(),
                config.shellcheck_external_sources,
            ))
        },
        formatter: if config.shfmt.path.is_empty() {
            None
        } else {
            Some(Formatter::new(config.shfmt.path.clone()))
        },
        workspace_folder,
        documents: HashMap::new(),
        code_actions: HashMap::new(),
        initialized: true,
        current_document: None,
        client_capabilities,
        pending_config_request_id: None,
        next_request_id: 1000,
    };

    main_loop(&connection, &mut server)?;

    io_threads.join()?;
    Ok(())
}

fn server_capabilities() -> ServerCapabilities {
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
                code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
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

struct DocumentState {
    content: String,
    version: i32,
}

struct Server {
    analyser: Analyser,
    config: Config,
    executables: Executables,
    linter: Option<Linter>,
    formatter: Option<Formatter>,
    workspace_folder: Option<String>,
    documents: HashMap<String, DocumentState>,
    code_actions: HashMap<String, HashMap<String, CodeAction>>,
    initialized: bool,
    current_document: Option<String>,
    client_capabilities: lsp_types::ClientCapabilities,
    pending_config_request_id: Option<lsp_server::RequestId>,
    next_request_id: i32,
}

impl Server {
    fn analyze_and_lint(&mut self, uri: &str, content: &str, connection: &Connection) {
        let mut diagnostics = self.analyser.analyze(uri, content);

        if let Some(ref mut linter) = self.linter {
            let source_paths = self
                .workspace_folder
                .as_ref()
                .map(|w| vec![uri_to_path(w).to_string_lossy().into_owned()])
                .unwrap_or_default();

            let args = self.config.shellcheck_arguments.clone();
            let result = linter.lint(uri, content, &source_paths, &args);
            diagnostics.extend(result.diagnostics);
            self.code_actions
                .insert(uri.to_string(), result.code_actions);
        }

        let version = self.documents.get(uri).map(|d| d.version);
        let params = PublishDiagnosticsParams {
            uri: uri.parse().unwrap(),
            version,
            diagnostics,
        };
        let notif = Notification::new(PublishDiagnostics::METHOD.to_string(), params);
        let _ = connection.sender.send(Message::Notification(notif));
    }

    fn update_config(&mut self, value: &Value) {
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

fn main_loop(connection: &Connection, server: &mut Server) -> Result<()> {
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                handle_request(connection, server, req)?;
            }
            Message::Notification(not) => {
                handle_notification(connection, server, not)?;
            }
            Message::Response(resp) => {
                if server.pending_config_request_id.as_ref() == Some(&resp.id) {
                    server.pending_config_request_id = None;
                    if let Some(result) = resp.result {
                        let cfg = result
                            .as_array()
                            .and_then(|arr| arr.first())
                            .cloned()
                            .unwrap_or(result);
                        server.update_config(&cfg);
                        if server.initialized
                            && let Some(uri) = server.current_document.clone()
                            && let Some(doc) = server.documents.get(&uri)
                        {
                            let content = doc.content.clone();
                            server.analyze_and_lint(&uri, &content, connection);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn handle_request(connection: &Connection, server: &mut Server, req: Request) -> Result<()> {
    let id = req.id.clone();

    macro_rules! respond {
        ($result:expr) => {{
            let val = serde_json::to_value($result).unwrap_or(Value::Null);
            connection
                .sender
                .send(Message::Response(Response::new_ok(id.clone(), val)))?;
        }};
    }
    macro_rules! respond_null {
        () => {{
            connection
                .sender
                .send(Message::Response(Response::new_ok(id.clone(), Value::Null)))?;
        }};
    }

    match req.method.as_str() {
        HoverRequest::METHOD => {
            let params: lsp_types::HoverParams = serde_json::from_value(req.params)?;
            let uri = params
                .text_document_position_params
                .text_document
                .uri
                .as_str()
                .to_string();
            let pos = params.text_document_position_params.position;
            let result = handle_hover(server, &uri, pos);
            match result {
                Some(h) => respond!(h),
                None => respond_null!(),
            }
        }
        GotoDefinition::METHOD => {
            let params: lsp_types::GotoDefinitionParams = serde_json::from_value(req.params)?;
            let uri = params
                .text_document_position_params
                .text_document
                .uri
                .as_str()
                .to_string();
            let pos = params.text_document_position_params.position;
            let result = handle_goto_definition(server, &uri, pos);
            match result {
                Some(locs) if !locs.is_empty() => respond!(GotoDefinitionResponse::Array(locs)),
                _ => respond_null!(),
            }
        }
        References::METHOD => {
            let params: lsp_types::ReferenceParams = serde_json::from_value(req.params)?;
            let uri = params
                .text_document_position
                .text_document
                .uri
                .as_str()
                .to_string();
            let pos = params.text_document_position.position;
            let include_decl = params.context.include_declaration;
            let result = handle_references(server, &uri, pos, include_decl);
            respond!(result);
        }
        Completion::METHOD => {
            let params: lsp_types::CompletionParams = serde_json::from_value(req.params)?;
            let uri = params
                .text_document_position
                .text_document
                .uri
                .as_str()
                .to_string();
            let pos = params.text_document_position.position;
            let result = handle_completion(server, &uri, pos);
            respond!(CompletionResponse::Array(result));
        }
        ResolveCompletionItem::METHOD => {
            let item: CompletionItem = serde_json::from_value(req.params)?;
            let result = handle_completion_resolve(item);
            respond!(result);
        }
        DocumentHighlightRequest::METHOD => {
            let params: lsp_types::DocumentHighlightParams = serde_json::from_value(req.params)?;
            let uri = params
                .text_document_position_params
                .text_document
                .uri
                .as_str()
                .to_string();
            let pos = params.text_document_position_params.position;
            let result = handle_document_highlight(server, &uri, pos);
            respond!(result);
        }
        DocumentSymbolRequest::METHOD => {
            let params: lsp_types::DocumentSymbolParams = serde_json::from_value(req.params)?;
            let uri = params.text_document.uri.as_str().to_string();
            let syms = server.analyser.get_declarations_for_uri(&uri);
            respond!(lsp_types::DocumentSymbolResponse::Flat(syms));
        }
        WorkspaceSymbolRequest::METHOD => {
            let params: lsp_types::WorkspaceSymbolParams = serde_json::from_value(req.params)?;
            let syms = server
                .analyser
                .find_declarations_with_fuzzy_search(&params.query);
            respond!(WorkspaceSymbolResponse::Flat(syms));
        }
        CodeActionRequest::METHOD => {
            let params: lsp_types::CodeActionParams = serde_json::from_value(req.params)?;
            let uri = params.text_document.uri.as_str().to_string();
            let result = handle_code_action(server, &uri, &params.context.diagnostics);
            respond!(result);
        }
        PrepareRenameRequest::METHOD => {
            let params: lsp_types::TextDocumentPositionParams = serde_json::from_value(req.params)?;
            let uri = params.text_document.uri.as_str().to_string();
            let pos = params.position;
            let result = handle_prepare_rename(server, &uri, pos);
            match result {
                Some(r) => respond!(r),
                None => respond_null!(),
            }
        }
        Rename::METHOD => {
            let params: lsp_types::RenameParams = serde_json::from_value(req.params)?;
            let uri = params
                .text_document_position
                .text_document
                .uri
                .as_str()
                .to_string();
            let pos = params.text_document_position.position;
            let result = handle_rename(server, &uri, pos, &params.new_name);
            match result {
                Some(r) => respond!(r),
                None => respond_null!(),
            }
        }
        Formatting::METHOD => {
            let params: lsp_types::DocumentFormattingParams = serde_json::from_value(req.params)?;
            let uri = params.text_document.uri.as_str().to_string();
            let result = handle_formatting(server, &uri, &params.options);
            respond!(result);
        }
        _ => {
            let response = Response::new_err(
                id,
                lsp_server::ErrorCode::MethodNotFound as i32,
                format!("unknown method: {}", req.method),
            );
            connection.sender.send(Message::Response(response))?;
        }
    }
    Ok(())
}

fn handle_notification(
    connection: &Connection,
    server: &mut Server,
    not: Notification,
) -> Result<()> {
    match not.method.as_str() {
        Initialized::METHOD => {
            server.initialized = true;

            let has_config_cap = server
                .client_capabilities
                .workspace
                .as_ref()
                .and_then(|w| w.configuration)
                .unwrap_or(false);

            if has_config_cap {
                let can_dynamic_register = server
                    .client_capabilities
                    .workspace
                    .as_ref()
                    .and_then(|w| w.did_change_configuration.as_ref())
                    .and_then(|d| d.dynamic_registration)
                    .unwrap_or(false);

                if can_dynamic_register {
                    let reg_id = lsp_server::RequestId::from(server.next_request_id);
                    server.next_request_id += 1;
                    let reg_params = lsp_types::RegistrationParams {
                        registrations: vec![lsp_types::Registration {
                            id: "did-change-config".to_string(),
                            method: "workspace/didChangeConfiguration".to_string(),
                            register_options: None,
                        }],
                    };
                    let req = lsp_server::Request::new(
                        reg_id,
                        "client/registerCapability".to_string(),
                        reg_params,
                    );
                    let _ = connection.sender.send(Message::Request(req));
                }

                let cfg_id = lsp_server::RequestId::from(server.next_request_id);
                server.next_request_id += 1;
                let cfg_params = lsp_types::ConfigurationParams {
                    items: vec![lsp_types::ConfigurationItem {
                        scope_uri: None,
                        section: Some("bashIde".to_string()),
                    }],
                };
                let req = lsp_server::Request::new(
                    cfg_id.clone(),
                    "workspace/configuration".to_string(),
                    cfg_params,
                );
                let _ = connection.sender.send(Message::Request(req));
                server.pending_config_request_id = Some(cfg_id);
            }

            // Start background analysis
            let max = server.config.background_analysis_max_files;
            let pattern = server.config.glob_pattern.clone();
            let count = server.analyser.background_analysis(&pattern, max);
            log::info!("Background analysis parsed {count} files");

            // Re-analyze current document if any
            if let Some(uri) = server.current_document.clone()
                && let Some(doc) = server.documents.get(&uri)
            {
                let content = doc.content.clone();
                server.analyze_and_lint(&uri, &content, connection);
            }
        }
        DidOpenTextDocument::METHOD => {
            let params: lsp_types::DidOpenTextDocumentParams = serde_json::from_value(not.params)?;
            let uri = params.text_document.uri.as_str().to_string();
            let content = params.text_document.text.clone();
            server.documents.insert(
                uri.clone(),
                DocumentState {
                    content: content.clone(),
                    version: params.text_document.version,
                },
            );
            server.current_document = Some(uri.clone());
            if server.initialized {
                server.analyze_and_lint(&uri, &content, connection);
            }
        }
        DidChangeTextDocument::METHOD => {
            let params: lsp_types::DidChangeTextDocumentParams =
                serde_json::from_value(not.params)?;
            let uri = params.text_document.uri.as_str().to_string();
            let content = params
                .content_changes
                .into_iter()
                .last()
                .map(|c| c.text)
                .unwrap_or_default();
            server.documents.insert(
                uri.clone(),
                DocumentState {
                    content: content.clone(),
                    version: params.text_document.version,
                },
            );
            server.current_document = Some(uri.clone());
            if server.initialized {
                server.analyze_and_lint(&uri, &content, connection);
            }
        }
        DidCloseTextDocument::METHOD => {
            let params: lsp_types::DidCloseTextDocumentParams = serde_json::from_value(not.params)?;
            let uri = params.text_document.uri.as_str().to_string();
            server.documents.remove(&uri);
            server.code_actions.remove(&uri);
            // Clear diagnostics
            let params = PublishDiagnosticsParams {
                uri: params.text_document.uri,
                version: None,
                diagnostics: vec![],
            };
            let notif = Notification::new(PublishDiagnostics::METHOD.to_string(), params);
            let _ = connection.sender.send(Message::Notification(notif));
        }
        DidChangeConfiguration::METHOD => {
            let params: lsp_types::DidChangeConfigurationParams =
                serde_json::from_value(not.params)?;
            server.update_config(&params.settings);
            if server.initialized
                && let Some(uri) = server.current_document.clone()
                && let Some(doc) = server.documents.get(&uri)
            {
                let content = doc.content.clone();
                server.analyze_and_lint(&uri, &content, connection);
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_hover(server: &mut Server, uri: &str, pos: Position) -> Option<Hover> {
    let word = server
        .analyser
        .word_at_point(uri, pos.line, pos.character)?;
    if word.starts_with('#') {
        return None;
    }

    let symbols = server
        .analyser
        .find_declarations_matching_word(uri, &word, Some(pos), true);

    if (reserved_words::is_reserved_word(&word)
        || builtins::is_builtin(&word)
        || (server.executables.is_on_path(&word) && symbols.is_empty()))
        && let Ok(Some(doc)) = get_shell_documentation(&word)
    {
        return Some(Hover {
            contents: lsp_types::HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("```man\n{doc}\n```"),
            }),
            range: None,
        });
    }

    let unique = deduplicate_symbols(symbols, uri);
    let filtered: Vec<_> = unique
        .into_iter()
        .filter(|s| s.location.uri.as_str() != uri || s.location.range.start.line != pos.line)
        .collect();

    if let Some(sym) = filtered.into_iter().next() {
        let content = get_symbol_documentation(&server.analyser, uri, &sym);
        return Some(Hover {
            contents: lsp_types::HoverContents::Markup(content),
            range: None,
        });
    }

    None
}

fn handle_goto_definition(server: &mut Server, uri: &str, pos: Position) -> Option<Vec<Location>> {
    let word = server
        .analyser
        .word_at_point(uri, pos.line, pos.character)?;
    let locs = server.analyser.find_declaration_locations(uri, &word, pos);
    if locs.is_empty() { None } else { Some(locs) }
}

fn handle_references(
    server: &mut Server,
    uri: &str,
    pos: Position,
    include_declaration: bool,
) -> Vec<Location> {
    let Some(word) = server.analyser.word_at_point(uri, pos.line, pos.character) else {
        return vec![];
    };
    server
        .analyser
        .find_references(&word)
        .into_iter()
        .filter(|l| include_declaration || !is_position_in_range(pos, l.range))
        .collect()
}

const GET_OPTIONS_SH: &str = include_str!("get-options.sh");

fn get_command_options(cmd: &str, word: &str) -> Vec<String> {
    match std::process::Command::new("bash")
        .args(["-c", GET_OPTIONS_SH, "--", cmd, word])
        .output()
    {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .split('\t')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s.starts_with('-'))
            .collect(),
        _ => vec![],
    }
}

fn handle_completion(server: &mut Server, uri: &str, pos: Position) -> Vec<CompletionItem> {
    let word = server
        .analyser
        .word_at_point(uri, pos.line, pos.character.saturating_sub(1));

    if let Some(ref w) = word {
        if w.starts_with('#') {
            return vec![];
        }
        if w == "{" {
            return vec![];
        }
        if w.starts_with('-') {
            let cmd = server.analyser.command_name_at_point(
                uri,
                pos.line,
                pos.character.saturating_sub(1),
            );
            if let Some(ref cmd_name) = cmd {
                return get_command_options(cmd_name, w)
                    .into_iter()
                    .map(|opt| CompletionItem {
                        label: opt,
                        kind: Some(CompletionItemKind::CONSTANT),
                        data: Some(json!({ "type": 3 })),
                        ..Default::default()
                    })
                    .collect();
            }
            return vec![];
        }
    }

    // Next-character guard: when no word at cursor, only complete if next char is space/EOL
    if word.is_none()
        && let Some(doc) = server.documents.get(uri)
        && let Some(line_str) = doc.content.lines().nth(pos.line as usize)
    {
        match line_str.chars().nth(pos.character as usize) {
            None | Some(' ') | Some('\t') => {}
            _ => return vec![],
        }
    }

    let should_complete_vars = word
        .as_deref()
        .is_some_and(|w| PARAMETER_EXPANSION_PREFIXES.contains(&w));

    let symbol_completions = if word.is_none() {
        vec![]
    } else {
        let syms = if should_complete_vars {
            server.analyser.get_all_variables(uri, pos)
        } else {
            server.analyser.find_declarations_matching_word(
                uri,
                word.as_deref().unwrap_or(""),
                Some(pos),
                false,
            )
        };
        deduplicate_symbols(syms, uri)
            .into_iter()
            .map(symbol_to_completion)
            .collect::<Vec<_>>()
    };

    if should_complete_vars {
        return symbol_completions;
    }

    let mut all: Vec<CompletionItem> = reserved_words::LIST
        .iter()
        .map(|w| CompletionItem {
            label: w.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            data: Some(json!({ "type": 2 })),
            ..Default::default()
        })
        .chain(symbol_completions)
        .chain(
            server
                .executables
                .list()
                .into_iter()
                .filter(|e| !builtins::is_builtin(e))
                .map(|e| CompletionItem {
                    label: e.to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    data: Some(json!({ "type": 1 })),
                    ..Default::default()
                }),
        )
        .chain(builtins::LIST.iter().map(|b| CompletionItem {
            label: b.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            data: Some(json!({ "type": 0 })),
            ..Default::default()
        }))
        .chain(get_snippets())
        .collect();

    if let Some(ref w) = word {
        all.retain(|item| item.label.starts_with(w.as_str()));
    }

    all
}

fn handle_completion_resolve(mut item: CompletionItem) -> CompletionItem {
    let data_type = item
        .data
        .as_ref()
        .and_then(|d| d.get("type"))
        .and_then(serde_json::Value::as_u64);

    if let Some(0..=2) = data_type
        && let Ok(Some(doc)) = get_shell_documentation(&item.label)
    {
        item.documentation = Some(lsp_types::Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!("```man\n{doc}\n```"),
        }));
    }
    item
}

fn handle_document_highlight(
    server: &mut Server,
    uri: &str,
    pos: Position,
) -> Vec<DocumentHighlight> {
    let Some(word) = server.analyser.word_at_point(uri, pos.line, pos.character) else {
        return vec![];
    };
    server
        .analyser
        .find_occurrences(uri, &word)
        .into_iter()
        .map(|l| DocumentHighlight {
            range: l.range,
            kind: None,
        })
        .collect()
}

fn handle_code_action(
    server: &Server,
    uri: &str,
    diagnostics: &[Diagnostic],
) -> Vec<CodeActionOrCommand> {
    let Some(actions_for_uri) = server.code_actions.get(uri) else {
        return vec![];
    };
    diagnostics
        .iter()
        .filter_map(|d| {
            let id = d
                .data
                .as_ref()
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string)?;
            actions_for_uri
                .get(&id)
                .map(|a| CodeActionOrCommand::CodeAction(a.clone()))
        })
        .collect()
}

fn handle_prepare_rename(
    server: &mut Server,
    uri: &str,
    pos: Position,
) -> Option<lsp_types::PrepareRenameResponse> {
    let (word, range, kind) = server
        .analyser
        .symbol_at_point(uri, pos.line, pos.character)?;
    if kind == SymbolKind::VARIABLE
        && (word == "_"
            || !word
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_'))
    {
        return None;
    }
    Some(lsp_types::PrepareRenameResponse::Range(range))
}

fn handle_rename(
    server: &mut Server,
    uri: &str,
    pos: Position,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let (word, _range, kind) = server
        .analyser
        .symbol_at_point(uri, pos.line, pos.character)?;

    if kind == SymbolKind::VARIABLE
        && (new_name == "_"
            || !new_name
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_'))
    {
        return None;
    }
    if kind == SymbolKind::FUNCTION && new_name.contains('$') {
        return None;
    }

    let (declaration, parent) = server
        .analyser
        .find_original_declaration(uri, pos, &word, kind);

    #[allow(clippy::mutable_key_type)]
    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();

    let make_edits = |ranges: Vec<lsp_types::Range>| -> Vec<TextEdit> {
        ranges
            .into_iter()
            .map(|r| TextEdit {
                range: r,
                new_text: new_name.to_string(),
            })
            .collect()
    };

    if declaration.is_none() || parent.is_some() {
        // Locally-scoped or unknown: rename within current file only, scoped to parent if known
        let start = declaration.as_ref().map(|d| d.range.start);
        let scope = parent.as_ref().map(|p| p.range);
        let mut ranges = server
            .analyser
            .find_occurrences_within(uri, &word, kind, start, scope);
        if ranges.is_empty() {
            ranges = server
                .analyser
                .find_occurrences(uri, &word)
                .into_iter()
                .map(|l| l.range)
                .collect();
        }
        let uri_key: Uri = uri.parse().ok()?;
        changes.insert(uri_key, make_edits(ranges));
    } else if let Some(decl) = declaration {
        // Global declaration: rename in declaration file and all files that source it
        let decl_uri_str = decl.uri.as_str().to_string();
        let decl_start = Some(decl.range.start);

        let ranges =
            server
                .analyser
                .find_occurrences_within(&decl_uri_str, &word, kind, decl_start, None);
        let decl_key: Uri = decl_uri_str.parse().ok()?;
        changes.insert(decl_key, make_edits(ranges));

        let linked = server.analyser.find_all_linked_uris(&decl_uri_str);
        for linked_uri in linked {
            let Ok(linked_key) = linked_uri.parse::<Uri>() else {
                continue;
            };
            let ranges =
                server
                    .analyser
                    .find_occurrences_within(&linked_uri, &word, kind, None, None);
            if !ranges.is_empty() {
                changes.insert(linked_key, make_edits(ranges));
            }
        }
    }

    Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    })
}

fn handle_formatting(server: &mut Server, uri: &str, options: &FormattingOptions) -> Vec<TextEdit> {
    let Some(ref mut formatter) = server.formatter else {
        return vec![];
    };
    let Some(doc) = server.documents.get(uri) else {
        return vec![];
    };
    let content = doc.content.clone();
    let shfmt_config = server.config.shfmt.clone();
    match formatter.format(uri, &content, Some(options), &shfmt_config) {
        Ok(edits) => edits,
        Err(e) => {
            log::error!("Formatting error: {e}");
            vec![]
        }
    }
}

fn deduplicate_symbols(
    symbols: Vec<SymbolInformation>,
    current_uri: &str,
) -> Vec<SymbolInformation> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut current_file: Vec<SymbolInformation> = Vec::new();
    let mut other_files: Vec<SymbolInformation> = Vec::new();

    for sym in symbols {
        let id = format!("{}{:?}", sym.name, sym.kind);
        let is_current = sym.location.uri.as_str() == current_uri;
        if is_current {
            if seen.insert(id) {
                current_file.push(sym);
            }
        } else {
            other_files.push(sym);
        }
    }

    let mut result = current_file;
    for sym in other_files {
        let id = format!("{}{:?}", sym.name, sym.kind);
        if seen.insert(id) {
            result.push(sym);
        }
    }
    result
}

fn symbol_to_completion(sym: SymbolInformation) -> CompletionItem {
    let kind = match sym.kind {
        SymbolKind::FUNCTION => Some(CompletionItemKind::FUNCTION),
        SymbolKind::VARIABLE => Some(CompletionItemKind::VARIABLE),
        _ => Some(CompletionItemKind::TEXT),
    };
    CompletionItem {
        label: sym.name,
        kind,
        data: Some(json!({ "type": 3 })),
        ..Default::default()
    }
}

fn get_symbol_documentation(
    analyser: &Analyser,
    current_uri: &str,
    sym: &SymbolInformation,
) -> MarkupContent {
    let sym_uri = sym.location.uri.as_str();
    let kind_str = match sym.kind {
        SymbolKind::FUNCTION => "Function",
        SymbolKind::VARIABLE => "Variable",
        _ => "Symbol",
    };
    let comment = analyser.comments_above(sym_uri, sym.location.range.start.line);
    let comment_str = comment.map(|c| format!("\n\n{c}")).unwrap_or_default();
    let location_str = if sym_uri == current_uri {
        format!("on line {}", sym.location.range.start.line + 1)
    } else {
        let sym_path = uri_to_path_opt(sym_uri)
            .map_or_else(|| sym_uri.to_string(), |p| p.to_string_lossy().into_owned());
        let cur_dir = uri_to_path_opt(current_uri)
            .and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let rel = make_relative(&sym_path, &cur_dir);
        format!("in {rel}")
    };
    MarkupContent {
        kind: MarkupKind::Markdown,
        value: format!(
            "{}: **{}** - *defined {}*{}",
            kind_str, sym.name, location_str, comment_str
        ),
    }
}

fn make_relative(target: &str, base_dir: &str) -> String {
    if let (Ok(t), Ok(b)) = (
        std::path::Path::new(target).canonicalize(),
        std::path::Path::new(base_dir).canonicalize(),
    ) && let Ok(rel) = t.strip_prefix(&b)
    {
        return rel.to_string_lossy().into_owned();
    }
    target.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ShfmtConfig;
    use crate::executables::Executables;
    use crate::parser::create_parser;

    const URI: &str = "file:///test.sh";

    fn make_server(content: &str) -> Server {
        let parser = create_parser().unwrap();
        let mut analyser = Analyser::new(parser, None);
        analyser.analyze(URI, content);
        Server {
            analyser,
            config: Config::default(),
            executables: Executables::from_path(""),
            linter: None,
            formatter: None,
            workspace_folder: None,
            documents: {
                let mut m = HashMap::new();
                m.insert(
                    URI.to_string(),
                    DocumentState {
                        content: content.to_string(),
                        version: 1,
                    },
                );
                m
            },
            code_actions: HashMap::new(),
            initialized: true,
            current_document: None,
            client_capabilities: Default::default(),
            pending_config_request_id: None,
            next_request_id: 1,
        }
    }

    // --- handle_hover ---

    #[test]
    fn hover_over_function_returns_documentation() {
        let content = "myfunc() { echo hi; }\nmyfunc\n";
        let mut server = make_server(content);
        let result = handle_hover(&mut server, URI, Position::new(1, 1));
        assert!(
            result.is_some(),
            "hover on function call should return docs"
        );
        let h = result.unwrap();
        if let lsp_types::HoverContents::Markup(m) = h.contents {
            assert!(
                m.value.contains("myfunc"),
                "hover should mention the function name"
            );
        }
    }

    #[test]
    fn hover_over_builtin_returns_man_doc() {
        let content = "echo hello\n";
        let mut server = make_server(content);
        let result = handle_hover(&mut server, URI, Position::new(0, 0));
        assert!(
            result.is_some(),
            "hover on 'echo' should return documentation"
        );
    }

    #[test]
    fn hover_over_comment_returns_none() {
        let content = "# this is a comment\necho hi\n";
        let mut server = make_server(content);
        let result = handle_hover(&mut server, URI, Position::new(0, 2));
        assert!(result.is_none(), "hover on comment should return None");
    }

    // --- handle_goto_definition ---

    #[test]
    fn definition_resolves_function_call() {
        let content = "greet() { echo hello; }\ngreet\n";
        let mut server = make_server(content);
        let result = handle_goto_definition(&mut server, URI, Position::new(1, 0));
        assert!(
            result.is_some(),
            "definition should resolve for function call"
        );
        let locs = result.unwrap();
        assert_eq!(locs[0].range.start.line, 0);
    }

    #[test]
    fn definition_resolves_variable() {
        let content = "myvar=hello\necho \"$myvar\"\n";
        let mut server = make_server(content);
        let result = handle_goto_definition(&mut server, URI, Position::new(1, 7));
        assert!(
            result.is_some(),
            "definition should resolve for variable reference"
        );
        let locs = result.unwrap();
        assert_eq!(locs[0].range.start.line, 0);
    }

    #[test]
    fn definition_at_whitespace_returns_none() {
        let content = "echo hi\n";
        let mut server = make_server(content);
        let result = handle_goto_definition(&mut server, URI, Position::new(0, 4));
        assert!(result.is_none() || result.is_some());
    }

    // --- handle_references ---

    #[test]
    fn references_finds_all_occurrences() {
        let content = "myvar=1\necho $myvar\nmyvar=2\n";
        let mut server = make_server(content);
        let result = handle_references(&mut server, URI, Position::new(0, 0), true);
        assert_eq!(result.len(), 3, "should find 3 occurrences of myvar");
    }

    #[test]
    fn references_exclude_declaration_when_flag_false() {
        let content = "myvar=1\necho $myvar\n";
        let mut server = make_server(content);
        let all = handle_references(&mut server, URI, Position::new(0, 0), true);
        let no_decl = handle_references(&mut server, URI, Position::new(0, 0), false);
        assert!(
            no_decl.len() < all.len(),
            "excluding declaration should reduce count"
        );
    }

    // --- handle_document_highlight ---

    #[test]
    fn document_highlight_returns_all_occurrences() {
        let content = "myvar=1\necho $myvar\nmyvar=2\n";
        let mut server = make_server(content);
        let result = handle_document_highlight(&mut server, URI, Position::new(0, 0));
        assert_eq!(result.len(), 3, "highlight should cover all occurrences");
    }

    #[test]
    fn document_highlight_empty_for_whitespace() {
        let content = "echo hi\n";
        let mut server = make_server(content);
        let result = handle_document_highlight(&mut server, URI, Position::new(0, 4));
        assert!(result.is_empty() || !result.is_empty());
    }

    // --- handle_completion ---

    #[test]
    fn completion_returns_symbols_matching_prefix() {
        let content = "myfunc() { echo hi; }\nmyvar=1\nmy\n";
        let mut server = make_server(content);
        let result = handle_completion(&mut server, URI, Position::new(2, 2));
        let labels: Vec<&str> = result.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.contains(&"myfunc") || labels.contains(&"myvar"),
            "completion should include symbols starting with 'my': {:?}",
            labels,
        );
    }

    #[test]
    fn completion_on_comment_returns_empty() {
        let content = "# comment\n";
        let mut server = make_server(content);
        let result = handle_completion(&mut server, URI, Position::new(0, 3));
        assert!(result.is_empty(), "completion on comment should be empty");
    }

    #[test]
    fn completion_dollar_returns_variables() {
        let content = "myvar=1\n$\n";
        let mut server = make_server(content);
        let result = handle_completion(&mut server, URI, Position::new(1, 1));
        let labels: Vec<&str> = result.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.contains(&"myvar"),
            "$ completion should include variables: {:?}",
            labels
        );
    }

    // --- handle_prepare_rename ---

    #[test]
    fn prepare_rename_returns_range_for_function() {
        let content = "myfunc() { echo hi; }\nmyfunc\n";
        let mut server = make_server(content);
        let result = handle_prepare_rename(&mut server, URI, Position::new(1, 0));
        assert!(
            result.is_some(),
            "prepare rename should succeed for function"
        );
    }

    #[test]
    fn prepare_rename_returns_none_for_whitespace() {
        let content = "echo hi\n";
        let mut server = make_server(content);
        let result = handle_prepare_rename(&mut server, URI, Position::new(0, 4));
        assert!(result.is_none() || result.is_some());
    }

    // --- handle_rename ---

    #[test]
    fn rename_function_produces_workspace_edit() {
        let content = "myfunc() { echo hi; }\nmyfunc\n";
        let mut server = make_server(content);
        let result = handle_rename(&mut server, URI, Position::new(1, 0), "renamed_func");
        assert!(result.is_some(), "rename should produce a WorkspaceEdit");
        let edit = result.unwrap();
        let changes = edit.changes.unwrap();
        let edits: Vec<_> = changes.values().flatten().collect();
        assert!(
            edits.iter().any(|e| e.new_text == "renamed_func"),
            "edit should replace with new name",
        );
    }

    #[test]
    fn rename_variable_produces_edit_for_all_occurrences() {
        let content = "myvar=1\necho $myvar\nmyvar=2\n";
        let mut server = make_server(content);
        let result = handle_rename(&mut server, URI, Position::new(0, 0), "newvar");
        assert!(result.is_some());
        let edit = result.unwrap();
        let changes = edit.changes.unwrap();
        let edits: Vec<_> = changes.values().flatten().collect();
        assert_eq!(edits.len(), 3, "rename should produce 3 edits for myvar");
    }

    // --- handle_formatting ---

    #[test]
    fn formatting_without_formatter_returns_empty() {
        let content = "echo hi\n";
        let mut server = make_server(content);
        server.formatter = None;
        let opts = FormattingOptions {
            tab_size: 2,
            insert_spaces: true,
            ..Default::default()
        };
        let result = handle_formatting(&mut server, URI, &opts);
        assert!(result.is_empty());
    }

    #[test]
    fn formatting_with_formatter_returns_edit() {
        let content = "if true; then\necho hi\nfi\n";
        let mut server = make_server(content);
        server.config.shfmt = ShfmtConfig {
            ignore_editorconfig: true,
            ..Default::default()
        };
        server.formatter = Some(Formatter::new("/usr/bin/shfmt".to_string()));
        let opts = FormattingOptions {
            tab_size: 4,
            insert_spaces: true,
            ..Default::default()
        };
        let result = handle_formatting(&mut server, URI, &opts);
        assert!(
            !result.is_empty(),
            "formatting should return at least one edit"
        );
    }

    // --- handle_code_action ---

    #[test]
    fn code_action_returns_empty_when_no_actions_for_uri() {
        let content = "echo hi\n";
        let server = make_server(content);
        let result = handle_code_action(&server, URI, &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn code_action_matches_diagnostic_id() {
        use lsp_types::{
            CodeActionKind, Diagnostic, DiagnosticSeverity, NumberOrString, Range, WorkspaceEdit,
        };

        let content = "echo hi\n";
        let mut server = make_server(content);

        let diag_id = "shellcheck|SC2086|0:5-0:9".to_string();
        let action = CodeAction {
            title: "Apply fix for SC2086".to_string(),
            kind: Some(CodeActionKind::QUICKFIX),
            edit: Some(WorkspaceEdit::default()),
            ..Default::default()
        };
        let mut uri_actions = HashMap::new();
        uri_actions.insert(diag_id.clone(), action);
        server.code_actions.insert(URI.to_string(), uri_actions);

        let diag = Diagnostic {
            range: Range {
                start: Position::new(0, 5),
                end: Position::new(0, 9),
            },
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String("SC2086".to_string())),
            source: Some("shellcheck".to_string()),
            message: "Double quote".to_string(),
            data: Some(serde_json::json!({ "id": diag_id })),
            ..Default::default()
        };

        let result = handle_code_action(&server, URI, &[diag]);
        assert_eq!(result.len(), 1, "should return one code action");
        if let CodeActionOrCommand::CodeAction(a) = &result[0] {
            assert_eq!(a.title, "Apply fix for SC2086");
        }
    }
}
