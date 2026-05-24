use crate::server::Server;
use lsp_types::{Position, SymbolKind, TextEdit, Uri, WorkspaceEdit};
use std::collections::HashMap;

pub fn handle_prepare_rename(
    server: &mut Server,
    uri: &str,
    pos: Position,
) -> Option<lsp_types::PrepareRenameResponse> {
    let (word, range, kind) = server.analyser.symbol_at_point(uri, pos.line, pos.character)?;
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

pub fn handle_rename(
    server: &mut Server,
    uri: &str,
    pos: Position,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let (word, _range, kind) = server.analyser.symbol_at_point(uri, pos.line, pos.character)?;

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

    let (declaration, parent) =
        server
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
        let mut ranges =
            server
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

        let ranges = server.analyser.find_occurrences_within(
            &decl_uri_str,
            &word,
            kind,
            decl_start,
            None,
        );
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
