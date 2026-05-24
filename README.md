# bashls

[![CI](https://github.com/k8s-1/bashls/actions/workflows/ci.yml/badge.svg)](https://github.com/k8s-1/bashls/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/bashls.svg)](https://crates.io/crates/bashls)

A Rust alternative to [bash-language-server](https://github.com/bash-lsp/bash-language-server), shipped as a single binary with no Node.js runtime and minimal dependencies to reduce supply chain attack surface.

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

- [shellcheck](https://github.com/koalaman/shellcheck) — diagnostics
- [shfmt](https://github.com/mvdan/sh) — formatting

```
cargo install bashls
```

Or build from source:

```
git clone https://github.com/k8s-1/bashls
cd bashls
cargo build --release
```

## Editor support

bashls works with any editor that supports LSP. Consult your editor's LSP documentation for implementation.

### Neovim

```lua
vim.lsp.config('bashls', {
  cmd = { 'bashls' },
  filetypes = { 'sh' },
  root_markers = { '.git' },
})
vim.lsp.enable('bashls')
```
