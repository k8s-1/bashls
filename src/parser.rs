use anyhow::Result;
use tree_sitter::Parser;

pub fn create_parser() -> Result<Parser> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_bash::LANGUAGE.into())
        .map_err(|e| anyhow::anyhow!("Failed to set bash language: {e}"))?;
    Ok(parser)
}
