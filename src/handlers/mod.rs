pub mod code_action;
pub mod completion;
pub mod formatting;
pub mod hover;
pub mod navigation;
pub mod rename;

pub(crate) use code_action::handle_code_action;
pub(crate) use completion::{handle_completion, handle_completion_resolve};
pub(crate) use formatting::handle_formatting;
pub(crate) use hover::handle_hover;
pub(crate) use navigation::{handle_document_highlight, handle_goto_definition, handle_references};
pub(crate) use rename::{handle_prepare_rename, handle_rename};

use lsp_types::SymbolInformation;
use std::collections::HashSet;

pub(crate) fn deduplicate_symbols(
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
