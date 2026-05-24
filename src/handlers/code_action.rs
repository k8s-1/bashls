use crate::server::Server;
use lsp_types::{CodeActionOrCommand, Diagnostic};

pub fn handle_code_action(
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
