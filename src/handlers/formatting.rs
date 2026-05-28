use crate::server::Server;
use lsp_types::{FormattingOptions, TextEdit};

pub fn handle_formatting(
    server: &mut Server,
    uri: &str,
    options: &FormattingOptions,
) -> Vec<TextEdit> {
    let Some(ref formatter) = server.formatter else {
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
