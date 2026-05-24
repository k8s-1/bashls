use crate::builtins;
use crate::reserved_words;
use crate::server::Server;
use crate::snippets::get_snippets;
use crate::util::sh::{get_command_options, get_shell_documentation};
use lsp_types::{CompletionItem, CompletionItemKind, Position, SymbolInformation, SymbolKind};
use serde_json::json;

use super::deduplicate_symbols;

const PARAMETER_EXPANSION_PREFIXES: &[&str] = &["$", "${"];

pub(crate) fn handle_completion(server: &mut Server, uri: &str, pos: Position) -> Vec<CompletionItem> {
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

pub(crate) fn handle_completion_resolve(mut item: CompletionItem) -> CompletionItem {
    let data_type = item
        .data
        .as_ref()
        .and_then(|d| d.get("type"))
        .and_then(serde_json::Value::as_u64);

    if let Some(0..=2) = data_type
        && let Ok(Some(doc)) = get_shell_documentation(&item.label)
    {
        item.documentation = Some(lsp_types::Documentation::MarkupContent(
            lsp_types::MarkupContent {
                kind: lsp_types::MarkupKind::Markdown,
                value: format!("```man\n{doc}\n```"),
            },
        ));
    }
    item
}

pub(crate) fn symbol_to_completion(sym: SymbolInformation) -> CompletionItem {
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
