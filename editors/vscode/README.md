# bashls

VS Code extension for [bashls](https://github.com/k8s-1/bashls), a Bash language server written in Rust.

## Requirements

If `bashls` isn't found on your `$PATH` (or the path set in `bashls.path`), the extension offers to download and install it automatically (Linux/macOS only — no prebuilt Windows binary). It's installed once and not auto-updated afterward; to get a newer version, install `bashls` yourself and set `bashls.path` to it.

Diagnostics and formatting also require [shellcheck](https://github.com/koalaman/shellcheck) and [shfmt](https://github.com/mvdan/sh) on `$PATH`.

## Settings

| Setting | Default | Description |
|---|---|---|
| `bashls.path` | `bashls` | Path to the bashls binary. |
| `bashls.shellcheckPath` | `shellcheck` | Path to shellcheck binary. |
| `bashls.shellcheckArguments` | `[]` | Additional arguments passed to shellcheck. |
| `bashls.shellcheckExternalSources` | `true` | Allow shellcheck to follow sourced files outside the workspace. |
| `bashls.shfmt.path` | `shfmt` | Path to shfmt binary. |
| `bashls.globPattern` | `**/*@(.sh\|.inc\|.bash\|.command)` | Files the server treats as bash. |
| `bashls.backgroundAnalysisMaxFiles` | `500` | Max files to analyse in background for workspace-wide features. |
| `bashls.includeAllWorkspaceSymbols` | `false` | Return functions and variables from all workspace files in symbol search. |
| `bashls.enableSourceErrorDiagnostics` | `false` | Show diagnostics when a `source`/`.` command cannot be resolved. |
| `bashls.trace.server` | `off` | Trace LSP communication with bashls. |
