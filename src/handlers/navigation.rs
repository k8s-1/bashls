use crate::server::Server;
use crate::util::lsp::is_position_in_range;
use lsp_types::{DocumentHighlight, Location, Position};

pub fn handle_goto_definition(
    server: &mut Server,
    uri: &str,
    pos: Position,
) -> Option<Vec<Location>> {
    let word = server.analyser.word_at_point(uri, pos.line, pos.character)?;
    let locs = server.analyser.find_declaration_locations(uri, &word, pos);
    if locs.is_empty() { None } else { Some(locs) }
}

pub fn handle_references(
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

pub fn handle_document_highlight(
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
