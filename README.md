# bashls

[![CI](https://github.com/k8s-1/bashls/actions/workflows/ci.yml/badge.svg)](https://github.com/k8s-1/bashls/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/bashls.svg)](https://crates.io/crates/bashls)

A Rust alternative to [bash-language-server](https://github.com/bash-lsp/bash-language-server), shipped as a single binary with no Node.js and minimal runtime dependencies.

<p align="center">
  <picture align="center">
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/k8s-1/bashls/main/assets/benchmark-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/k8s-1/bashls/main/assets/benchmark-light.svg">
    <img alt="Benchmark results comparing bashls to bash-language-server." src="https://raw.githubusercontent.com/k8s-1/bashls/main/assets/benchmark-dark.svg">
  </picture>
</p>

## Features

- Hover documentation
- Completions (variables, functions, executables, builtins, snippets)
- Jump to definition
- Find references
- Rename
- Document and workspace symbols
- Diagnostics via [shellcheck](https://github.com/koalaman/shellcheck)
- Formatting via [shfmt](https://github.com/mvdan/sh)

## Installation

Diagnostics and formatting require additional tools:

- [shellcheck](https://github.com/koalaman/shellcheck)
- [shfmt](https://github.com/mvdan/sh)

#### Via cargo
```
cargo install bashls
```

#### Pre-built binary
Download for your platform from the [releases page](https://github.com/k8s-1/bashls/releases), extract, and place `bashls` somewhere on your `$PATH`.

#### From source
```
git clone https://github.com/k8s-1/bashls
cd bashls
cargo build --release
```

## Configuration

bashls works with any editor that supports LSP. Consult your editor's LSP documentation for implementation.

Settings can be provided as LSP initialization options (under `bashIde`) or as environment variables. LSP settings take effect at runtime and can be updated via `workspace/didChangeConfiguration`.

| Env var | LSP setting (`bashIde.*`) | Default | Notes |
|---|---|---|---|
| `SHELLCHECK_PATH` | `shellcheckPath` | `shellcheck` | Set to empty string to disable |
| *(see [shellcheck](https://github.com/koalaman/shellcheck))* | `shellcheckArguments` | `[]` | Additional shellcheck arguments |
| `SHFMT_PATH` | `shfmt.path` | `shfmt` | Set to empty string to disable |
| *(see [shfmt](https://github.com/mvdan/sh))* | `shfmt.*` | | Remaining shfmt options |
| `GLOB_PATTERN` | `globPattern` | `**/*@(.sh,.inc,.bash,.command)` | Files the server treats as bash |
| `BACKGROUND_ANALYSIS_MAX_FILES` | `backgroundAnalysisMaxFiles` | `500` | Max files to analyse in background for workspace-wide features |
| `INCLUDE_ALL_WORKSPACE_SYMBOLS` | `includeAllWorkspaceSymbols` | `false` | Return functions and variables from all workspace files in symbol search, not just open files |
| `ENABLE_SOURCE_ERROR_DIAGNOSTICS` | `enableSourceErrorDiagnostics` | `false` | Show diagnostics when a `source`/`.` command cannot be resolved |
| `BASH_IDE_LOG_LEVEL` | `logLevel` | `info` | |

### Neovim

```lua
vim.lsp.config('bashls', {
  cmd = { 'bashls' },
  filetypes = { 'sh' },
  root_markers = { '.git' },
})
vim.lsp.enable('bashls')
```
