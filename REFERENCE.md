# Reference

## Folder structure

```
src/
├── main.rs, lib.rs     entry point and crate root
├── analyser.rs         document store, tree-sitter parsing, symbol lookup
├── parser.rs           tree-sitter Parser init for bash
├── config.rs           config from env vars
├── server/             LSP message loop, dispatch, Server/DocumentState
├── handlers/           one handler per LSP feature (completion, hover, navigation, rename, code_action, formatting)
├── shellcheck/         diagnostics via shellcheck
├── shfmt/              formatting via shfmt
└── util/               shared helpers (declarations, sourcing, tree-sitter, LSP types, fs, shebang)
```

## Architecture diagram

```mermaid
flowchart TD
    Editor["Editor (Neovim, Helix, Zed, Emacs)"] <-->|LSP over stdio| Dispatch

    subgraph bashls
        Dispatch["server/dispatch.rs<br/>message loop + request dispatch"]
        State["server/state.rs<br/>Server / DocumentState"]
        Analyser["analyser.rs<br/>document store, tree-sitter parsing, symbol lookup"]
        Parser["parser.rs<br/>tree-sitter Parser (bash grammar)"]

        Dispatch --> State
        Dispatch --> Handlers
        State --> Analyser
        Analyser --> Parser

        subgraph Handlers["handlers/"]
            Completion[completion.rs]
            Hover[hover.rs]
            Navigation[navigation.rs]
            Rename[rename.rs]
            CodeAction[code_action.rs]
            Formatting[formatting.rs]
        end

        Handlers --> Analyser

        Formatting --> Shfmt["shfmt/<br/>formatting via shfmt"]
        CodeAction --> Shellcheck["shellcheck/<br/>diagnostics via shellcheck"]

        Util["util/<br/>declarations, sourcing,<br/>tree_sitter, lsp, shebang, fs, sh"]
        Analyser --> Util
        Handlers --> Util
    end

    Shfmt -->|subprocess| ShfmtBin[(shfmt binary)]
    Shellcheck -->|subprocess| ShellcheckBin[(shellcheck binary)]
```
