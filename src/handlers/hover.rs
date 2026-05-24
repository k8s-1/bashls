use crate::analyser::Analyser;
use crate::builtins;
use crate::reserved_words;
use crate::server::Server;
use crate::util::fs::{make_relative, uri_to_path_opt};
use crate::util::sh::get_shell_documentation;
use lsp_types::{Hover, MarkupContent, MarkupKind, Position, SymbolInformation, SymbolKind};

use super::deduplicate_symbols;

pub fn handle_hover(server: &mut Server, uri: &str, pos: Position) -> Option<Hover> {
    let word = server.analyser.word_at_point(uri, pos.line, pos.character)?;
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

pub fn get_symbol_documentation(
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
