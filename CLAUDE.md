## Architecture

- `src/main.rs` — entry point; dispatches `start` or `get-options` subcommands
- `src/server.rs` — LSP request dispatch and handler functions (`handle_hover`, `handle_completion`, `handle_rename`, etc.)
- `src/analyser.rs` — document store, tree-sitter parsing, symbol lookup
- `src/parser.rs` — tree-sitter `Parser` initialization for bash
- `src/config.rs` — reads config from env vars at startup
- `src/executables.rs` — PATH executable discovery
- `src/builtins.rs` — bash builtin list and `is_builtin` lookup
- `src/reserved_words.rs` — bash reserved word list and `is_reserved_word` lookup
- `src/shellcheck/` — shellcheck integration (linting)
- `src/shfmt/` — shfmt integration (formatting)
- `src/snippets.rs` — completion snippets
- `src/util/declarations.rs` — variable/function declaration extraction
- `src/util/sourcing.rs` — `source`/`.` command resolution
- `src/util/tree_sitter.rs` — `position_to_point`, `node_range`, tree-sitter helpers
- `src/util/lsp.rs` — LSP type conversion helpers
- `src/util/shebang.rs` — shebang detection
- `src/util/fs.rs` — URI↔path conversion (`uri_to_path`, `path_to_uri`)
- `src/util/sh.rs` — shell documentation via `man` / `bash --help`
- `bash-language-server/` — the original TypeScript implementation (reference)

## Key patterns

- Uses `lsp-server` crate for the LSP transport layer (not async; no Tokio).
- Tree-sitter is used for parsing; `Analyser` holds `AnalyzedDocument { source, tree, global_declarations, sourced_uris, source_commands }` keyed by URI string.
- `position_to_point` / `node_range` in `src/util/tree_sitter.rs` convert between LSP `Position`/`Range` and tree-sitter `Point`.

## Build and test

```
just build    # cargo build --release
just test     # cargo test
just lint     # cargo clippy -- -D warnings
just fmt      # cargo fmt
```
