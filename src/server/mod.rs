mod dispatch;
mod state;

pub use state::{DocumentState, Server};

use std::collections::HashMap;

use crate::analyser::Analyser;
use crate::config::Config;
use crate::executables::Executables;
use crate::parser::create_parser;
use crate::shellcheck::Linter;
use crate::shfmt::Formatter;
use anyhow::Result;
use lsp_server::{Connection, Message};

use dispatch::{handle_notification, handle_request};
use state::{LintResult, server_capabilities};

fn check_runtime_deps() {
    let bash_ok = std::process::Command::new("bash")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
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
        .is_ok_and(|o| o.status.success());
    if !completion_ok {
        eprintln!("bashls: warning: bash-completion not found — option completions unavailable");
    }

    let shellcheck_ok = std::process::Command::new("shellcheck")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !shellcheck_ok {
        eprintln!("bashls: warning: shellcheck not found — diagnostics unavailable");
    }

    let shfmt_ok = std::process::Command::new("shfmt")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
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

    let client_capabilities = init_params.capabilities;

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
            Some(Formatter::new(config.shfmt.path))
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

    let (lint_tx, lint_rx) = crossbeam_channel::unbounded::<LintResult>();
    main_loop(&connection, &mut server, &lint_tx, &lint_rx)?;

    io_threads.join()?;
    Ok(())
}

fn main_loop(
    connection: &Connection,
    server: &mut Server,
    lint_tx: &crossbeam_channel::Sender<LintResult>,
    lint_rx: &crossbeam_channel::Receiver<LintResult>,
) -> Result<()> {
    loop {
        crossbeam_channel::select! {
            recv(connection.receiver) -> msg => {
                match msg? {
                    Message::Request(req) => {
                        if connection.handle_shutdown(&req)? {
                            return Ok(());
                        }
                        handle_request(connection, server, req)?;
                    }
                    Message::Notification(not) => {
                        handle_notification(connection, server, not, lint_tx)?;
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
                                    server.analyze_and_lint(&uri, &content, connection, lint_tx);
                                }
                            }
                        }
                    }
                }
            }
            recv(lint_rx) -> result => {
                if let Ok(result) = result {
                    server.apply_lint_result(result, &connection.sender);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DocumentState, Server};
    use crate::analyser::Analyser;
    use crate::config::{Config, ShfmtConfig};
    use crate::executables::Executables;
    use crate::handlers::{
        handle_code_action, handle_completion, handle_completion_resolve,
        handle_document_highlight, handle_formatting, handle_goto_definition, handle_hover,
        handle_prepare_rename, handle_references, handle_rename,
    };
    use crate::parser::create_parser;
    use crate::shfmt::Formatter;
    use lsp_types::CodeAction;
    use serde_json::json;
    use std::collections::HashMap;

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
        let result = handle_hover(&mut server, URI, lsp_types::Position::new(1, 1));
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
        let result = handle_hover(&mut server, URI, lsp_types::Position::new(0, 0));
        assert!(
            result.is_some(),
            "hover on 'echo' should return documentation"
        );
    }

    #[test]
    fn hover_over_comment_returns_none() {
        let content = "# this is a comment\necho hi\n";
        let mut server = make_server(content);
        let result = handle_hover(&mut server, URI, lsp_types::Position::new(0, 2));
        assert!(result.is_none(), "hover on comment should return None");
    }

    // --- handle_goto_definition ---

    #[test]
    fn definition_resolves_function_call() {
        let content = "greet() { echo hello; }\ngreet\n";
        let mut server = make_server(content);
        let result = handle_goto_definition(&mut server, URI, lsp_types::Position::new(1, 0));
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
        let result = handle_goto_definition(&mut server, URI, lsp_types::Position::new(1, 7));
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
        let result = handle_goto_definition(&mut server, URI, lsp_types::Position::new(0, 4));
        assert!(result.is_none());
    }

    // --- handle_references ---

    #[test]
    fn references_finds_all_occurrences() {
        let content = "myvar=1\necho $myvar\nmyvar=2\n";
        let mut server = make_server(content);
        let result = handle_references(&mut server, URI, lsp_types::Position::new(0, 0), true);
        assert_eq!(result.len(), 3, "should find 3 occurrences of myvar");
    }

    #[test]
    fn references_exclude_declaration_when_flag_false() {
        let content = "myvar=1\necho $myvar\n";
        let mut server = make_server(content);
        let all = handle_references(&mut server, URI, lsp_types::Position::new(0, 0), true);
        let no_decl = handle_references(&mut server, URI, lsp_types::Position::new(0, 0), false);
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
        let result = handle_document_highlight(&mut server, URI, lsp_types::Position::new(0, 0));
        assert_eq!(result.len(), 3, "highlight should cover all occurrences");
    }

    #[test]
    fn document_highlight_empty_for_whitespace() {
        let content = "echo hi\n";
        let mut server = make_server(content);
        let result = handle_document_highlight(&mut server, URI, lsp_types::Position::new(0, 4));
        assert!(result.is_empty());
    }

    // --- handle_completion ---

    #[test]
    fn completion_returns_symbols_matching_prefix() {
        let content = "myfunc() { echo hi; }\nmyvar=1\nmy\n";
        let mut server = make_server(content);
        let result = handle_completion(&mut server, URI, lsp_types::Position::new(2, 2));
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
        let result = handle_completion(&mut server, URI, lsp_types::Position::new(0, 3));
        assert!(result.is_empty(), "completion on comment should be empty");
    }

    #[test]
    fn completion_dollar_returns_variables() {
        let content = "myvar=1\n$\n";
        let mut server = make_server(content);
        let result = handle_completion(&mut server, URI, lsp_types::Position::new(1, 1));
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
        let result = handle_prepare_rename(&mut server, URI, lsp_types::Position::new(1, 0));
        assert!(
            result.is_some(),
            "prepare rename should succeed for function"
        );
    }

    #[test]
    fn prepare_rename_returns_none_for_whitespace() {
        let content = "echo hi\n";
        let mut server = make_server(content);
        let result = handle_prepare_rename(&mut server, URI, lsp_types::Position::new(0, 4));
        assert!(result.is_none());
    }

    // --- handle_rename ---

    #[test]
    fn rename_function_produces_workspace_edit() {
        let content = "myfunc() { echo hi; }\nmyfunc\n";
        let mut server = make_server(content);
        let result = handle_rename(
            &mut server,
            URI,
            lsp_types::Position::new(1, 0),
            "renamed_func",
        );
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
        let result = handle_rename(&mut server, URI, lsp_types::Position::new(0, 0), "newvar");
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
        let opts = lsp_types::FormattingOptions {
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
        let opts = lsp_types::FormattingOptions {
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
    fn hover_variable_shows_variable_documentation() {
        let content = "myvar=hello\necho $myvar\n";
        let mut server = make_server(content);
        let result = handle_hover(&mut server, URI, lsp_types::Position::new(1, 7));
        assert!(
            result.is_some(),
            "hover on variable reference should return docs"
        );
        if let lsp_types::HoverContents::Markup(m) = result.unwrap().contents {
            assert!(m.value.contains("myvar"));
        }
    }

    #[test]
    fn hover_unknown_word_returns_none() {
        let content = "undeclared_xyz\n";
        let mut server = make_server(content);
        let result = handle_hover(&mut server, URI, lsp_types::Position::new(0, 0));
        assert!(
            result.is_none(),
            "hover on unknown word with no docs should return None"
        );
    }

    #[test]
    fn references_on_whitespace_returns_empty() {
        let content = "myvar=1\necho $myvar\n";
        let mut server = make_server(content);
        let result = handle_references(&mut server, URI, lsp_types::Position::new(0, 6), true);
        assert!(result.is_empty());
    }

    #[test]
    fn rename_rejects_dollar_in_function_name() {
        let content = "myfunc() { echo hi; }\nmyfunc\n";
        let mut server = make_server(content);
        let result = handle_rename(&mut server, URI, lsp_types::Position::new(1, 0), "new$func");
        assert!(
            result.is_none(),
            "rename to name containing $ should be rejected"
        );
    }

    #[test]
    fn rename_rejects_underscore_as_variable_name() {
        let content = "myvar=1\necho $myvar\n";
        let mut server = make_server(content);
        let result = handle_rename(&mut server, URI, lsp_types::Position::new(0, 0), "_");
        assert!(
            result.is_none(),
            "rename variable to bare _ should be rejected"
        );
    }

    #[test]
    fn completion_resolve_builtin_adds_documentation() {
        use lsp_types::CompletionItem;
        let item = CompletionItem {
            label: "echo".to_string(),
            data: Some(serde_json::json!({ "type": 0 })),
            ..Default::default()
        };
        let resolved = handle_completion_resolve(item);
        assert!(
            resolved.documentation.is_some(),
            "resolve for a builtin should attach documentation"
        );
    }

    #[test]
    fn completion_resolve_user_symbol_unchanged() {
        use lsp_types::CompletionItem;
        let item = CompletionItem {
            label: "myfunc".to_string(),
            data: Some(serde_json::json!({ "type": 3 })),
            ..Default::default()
        };
        let resolved = handle_completion_resolve(item);
        assert!(resolved.documentation.is_none());
    }

    #[test]
    fn update_config_disables_linter_when_path_empty() {
        let content = "echo hi\n";
        let mut server = make_server(content);
        server.linter = Some(crate::shellcheck::Linter::new(
            "shellcheck".to_string(),
            false,
        ));
        let cfg = serde_json::json!({ "shellcheckPath": "" });
        server.update_config(&cfg);
        assert!(server.linter.is_none());
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
                start: lsp_types::Position::new(0, 5),
                end: lsp_types::Position::new(0, 9),
            },
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String("SC2086".to_string())),
            source: Some("shellcheck".to_string()),
            message: "Double quote".to_string(),
            data: Some(json!({ "id": diag_id })),
            ..Default::default()
        };

        let result = handle_code_action(&server, URI, &[diag]);
        assert_eq!(result.len(), 1, "should return one code action");
        if let lsp_types::CodeActionOrCommand::CodeAction(a) = &result[0] {
            assert_eq!(a.title, "Apply fix for SC2086");
        }
    }
}
