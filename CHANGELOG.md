## [unreleased]

### 🐛 Bug Fixes

- *(bench)* Accurate methodology and updated benchmark numbers

### 📚 Documentation

- Move gif to top of README
- Slow down demo.gif by 2x
- Update README motivation
- Tighten benchmark SVG layout
- Remove stale latency methodology description from README
- Update benchmark numbers (100x startup, 10x memory)
- Document cold start methodology and conservative 1600 ms figure
- Replace em dash with parens in benchmark note

### ⚙️ Miscellaneous Tasks

- Exclude examples from published crate
- Remove redundant white space char in comment
- *(bench)* Remove latency measurement
## [0.2.2] - 2026-05-25

### 🐛 Bug Fixes

- Pedantic clippy warnings

### 💼 Other

- Add Rust LSP integration benchmark

### 📚 Documentation

- Update benchmark SVGs with real-session numbers
- Add latency panel to benchmark SVGs
- Resize benchmark SVG bars and simplify latency label
- Drop latency panel from benchmark SVG
- Update benchmark SVGs with oh-my-bash load (50 files)
- Add benchmark methodology note to README
- Clarify benchmark methodology wording
- Clarify latency metric in benchmark note
- Remove benchmark methodology note from README
- Make benchmark SVG text bold
- Increase benchmark SVG font sizes
- Fix legend spacing in benchmark SVG
- Adjust legend spacing
- Add motivation, license, refresh benchmark numbers
- Add demo GIF

### ⚙️ Miscellaneous Tasks

- Add minimal crates.io readme
- Bump version to 0.2.2
## [0.2.1] - 2026-05-25

### ⚙️ Miscellaneous Tasks

- Bump version to 0.2.0
- Deny warnings globally via .cargo/config.toml
- Deny warnings per-crate instead of globally via RUSTFLAGS
- Bump version to 0.2.1
## [0.2.0] - 2026-05-25

### 📚 Documentation

- Correct benchmark ratio
- Add init_options example to Neovim config
- Comment out optional init_options in Neovim example
- Update install instruction

### ⚡ Performance

- Run shellcheck in background thread to unblock LSP request handling

### ⚙️ Miscellaneous Tasks

- Bump version to 0.2.0
## [0.1.10] - 2026-05-24

### 🚜 Refactor

- Split server.rs into server/{mod,state,dispatch}.rs

### 📚 Documentation

- Update README.md install

### 🧪 Testing

- Improve coverage across server, analyser, and declarations

### ⚙️ Miscellaneous Tasks

- Remove unused test fixtures ported from TypeScript implementation
- Bump version to 0.1.10
## [0.1.9] - 2026-05-24

### 🚜 Refactor

- Split server handlers into dedicated modules
- Replace pub(crate) with pub throughout handlers
- Improve variable names and apply strict clippy fixes

### ⚙️ Miscellaneous Tasks

- Update actions/checkout to v5 for Node.js 24 support
- Update actions/checkout to v6
- Bump version to 0.1.9
## [0.1.8] - 2026-05-24

### 📚 Documentation

- Replace config table with list
- Move editor support out of configuration section
- Use three-column table for configuration
- Restore accurate glob pattern default
- Simplify env var example in config section
- Prioritize pre-built binary in install recommends

### ⚙️ Miscellaneous Tasks

- Bump version to 0.1.8
## [0.1.7] - 2026-05-24

### 🚀 Features

- Probe shfmt/shellcheck at startup and warn if not found

### 🐛 Bug Fixes

- Update tests to reflect startup probe behaviour

### 📚 Documentation

- Add configuration reference table to README

### ⚙️ Miscellaneous Tasks

- Cleanup README configuration
- Fmt
- Add concurrency groups to prevent parallel ci runs
- Bump version to 0.1.7
## [0.1.6] - 2026-05-24

### ⚙️ Miscellaneous Tasks

- Update README install instructions
- Update README install instructions
- Adjust header size in install instructions
- Update README, make install dependencies more concise
- Add -h / -v flags
- Bump version to 0.1.6
## [0.1.5] - 2026-05-24

### ⚙️ Miscellaneous Tasks

- Add message to signed tag
- Bump version to 0.1.5
## [0.1.4] - 2026-05-24

### ⚙️ Miscellaneous Tasks

- Update Cargo.lock
- Add release workflow and drop Windows path handling
- Bump version to 0.1.4
## [0.1.3] - 2026-05-24

### ⚙️ Miscellaneous Tasks

- Release v0.1.2
- Add Cargo.lock
- Update benchmark SVGs with current measurements
- Update Cargo.toml for install speed
- Update Cargo.toml version
## [0.1.2] - 2026-05-24

### ⚙️ Miscellaneous Tasks

- Rename bls to bashls in docs, templates, and assets
- Update README opening statement
- Update README opening statement
## [0.1.1] - 2026-05-24

### 🚀 Features

- Implement bash language server in Rust

### 📚 Documentation

- Add CLAUDE.md with architecture overview
- Add CONTRIBUTING.md
- Require Co-Authored-By trailer for AI-assisted commits
- Simplify CONTRIBUTING to use just ci
- Update README description
- Add crates.io badge to README

### 🧪 Testing

- Add integration, linter, formatter, and sourcing test suites

### ⚙️ Miscellaneous Tasks

- Add README, LICENSE, Neovim contrib config, and benchmarks
- Add GitHub Actions workflow for PRs and main branch
- Run only on pull requests
- Drop cargo-audit, install shellcheck and shfmt via apt
- Add rust-toolchain.toml and GitHub issue/PR templates
- Add crates.io metadata and exclude dev files from package
- Add CI on main push and README status
- Optimize release profile
- Rename crate from bls to bashls
- Skip CI on markdown-only pushes to main
- Only trigger on src, tests, and Cargo files
- Add workflow_dispatch and fix README image paths
- Bump version to 0.1.1
