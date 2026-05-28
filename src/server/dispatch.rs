use anyhow::Result;
use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::notification::Notification as _;
use lsp_types::request::Request as _;
use lsp_types::{
    CompletionResponse, GotoDefinitionResponse, PublishDiagnosticsParams, WorkspaceSymbolResponse,
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
use serde_json::Value;

use crate::handlers::{
    handle_code_action, handle_completion, handle_completion_resolve, handle_document_highlight,
    handle_formatting, handle_goto_definition, handle_hover, handle_prepare_rename,
    handle_references, handle_rename,
};

use super::state::{DocumentState, Server};

pub(super) fn handle_request(
    connection: &Connection,
    server: &mut Server,
    req: Request,
) -> Result<()> {
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
            match handle_hover(server, &uri, pos) {
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
            match handle_goto_definition(server, &uri, pos) {
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
            respond!(handle_references(server, &uri, pos, include_decl));
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
            respond!(CompletionResponse::Array(handle_completion(
                server, &uri, pos
            )));
        }
        ResolveCompletionItem::METHOD => {
            let item: lsp_types::CompletionItem = serde_json::from_value(req.params)?;
            respond!(handle_completion_resolve(item));
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
            respond!(handle_document_highlight(server, &uri, pos));
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
            respond!(handle_code_action(
                server,
                &uri,
                &params.context.diagnostics
            ));
        }
        PrepareRenameRequest::METHOD => {
            let params: lsp_types::TextDocumentPositionParams = serde_json::from_value(req.params)?;
            let uri = params.text_document.uri.as_str().to_string();
            let pos = params.position;
            match handle_prepare_rename(server, &uri, pos) {
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
            match handle_rename(server, &uri, pos, &params.new_name) {
                Some(r) => respond!(r),
                None => respond_null!(),
            }
        }
        Formatting::METHOD => {
            let params: lsp_types::DocumentFormattingParams = serde_json::from_value(req.params)?;
            let uri = params.text_document.uri.as_str().to_string();
            respond!(handle_formatting(server, &uri, &params.options));
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

pub(super) fn handle_notification(
    connection: &Connection,
    server: &mut Server,
    not: Notification,
    lint_tx: &crossbeam_channel::Sender<super::state::LintResult>,
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

            let max = server.config.background_analysis_max_files;
            let pattern = server.config.glob_pattern.clone();
            let count = server.analyser.background_analysis(&pattern, max);
            log::info!("Background analysis parsed {count} files");

            if let Some(uri) = server.current_document.clone()
                && let Some(doc) = server.documents.get(&uri)
            {
                let content = doc.content.clone();
                server.analyze_and_lint(&uri, &content, connection, lint_tx);
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
                server.analyze_and_lint(&uri, &content, connection, lint_tx);
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
                server.analyze_and_lint(&uri, &content, connection, lint_tx);
            }
        }
        DidCloseTextDocument::METHOD => {
            let params: lsp_types::DidCloseTextDocumentParams = serde_json::from_value(not.params)?;
            let uri = params.text_document.uri.as_str().to_string();
            server.documents.remove(&uri);
            server.code_actions.remove(&uri);
            server.analyser.remove(&uri);
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
                server.analyze_and_lint(&uri, &content, connection, lint_tx);
            }
        }
        _ => {}
    }
    Ok(())
}
