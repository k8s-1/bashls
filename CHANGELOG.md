## [0.2.8] - 2026-06-04

### 🚀 Features

- Add --log-level flag, default to error, restore internal logs to warn
- Validate --log-level values and document flag in README

### 🐛 Bug Fixes

- Editorconfig glob sections matching all files instead of extension
- Use path_to_uri for sourced file URIs to ensure percent-encoding
- Don't use ? inside root_paths loop in resolve_sourced_uri
- Reject malformed localhost URIs in uri_to_path_opt
- Six correctness bugs across analyser, navigation, shellcheck, and shfmt
- Two correctness bugs in declarations
- Demote syntax-error log from warn to debug
- Propagate shellcheck stdin write error instead of discarding it
- Don't fall back to file-wide rename when a scope is known
- Avoid panic in sourcing when string node contains invalid UTF-8
- Match editorconfig sections with leading glob stars correctly
- Glob patterns without wildcards now match as literal suffixes
- Reduce log noise from per-request and per-file warn/error logs
- Demote shellcheck run error from warn to debug

### 🚜 Refactor

- Inline is_definition/is_reference helpers and simplify completion guards
- Remove unreachable unwrap_or in completion symbol lookup
- Split find_occurrences_within_tree into variable and function helpers
- Replace comment_re closure with nested fn comment_text in comments_above

### 📚 Documentation

- Add RUST_LOG=debug instructions to bug report template and contributing guide
- Add --version and --help to CLI flags table in README
- Drop --version and --help from CLI flags table
- Trim --log-level description

### ⚙️ Miscellaneous Tasks

- Format and changelog
- Remove redundant comment
- Update log dependency
- Rewrite let..else in sourcing.rs to ? for clippy error
## [0.2.7] - 2026-05-29

### 📚 Documentation

- Improve README intro and alt text for SEO
- Improve README intro and alt text for SEO

### ⚙️ Miscellaneous Tasks

- Update dependencies
- Bump version to 0.2.6
- Bump version to 0.2.7
## [0.2.5] - 2026-05-28

### 🐛 Bug Fixes

- Evict analyser document on DidCloseTextDocument

### 🚜 Refactor

- Readability improvements across analyser and utils
- Misc readability improvements

### 📚 Documentation

- Update benchmark numbers with fresh measurements
- Shellcheck & shfmt are gracefully disabled if not found, no need to set path to empty

### ⚡ Performance

- Use HashSet for seen_ranges deduplication in find_occurrences
- Remove redundant sourced_uris field and filter completions eagerly

### 🧪 Testing

- Add test for Analyser::remove

### ⚙️ Miscellaneous Tasks

- Revert demo.gif 2x slowdown
- Remove dead logLevel config option
- Refactor lsp_bench nested for
- Deterministic source URI traversal order in find_all_sourced_uris
- Remove speculative comment from get_text_edits
- Remove unnecessary mut from formatter test bindings
- Bump version to 0.2.5

### ◀️ Revert

- Restore original completion retain pattern for readability
## [0.2.4] - 2026-05-27

### 🐛 Bug Fixes

- Use inclusive bounds in in_ignored_range for single-line scopes
- Eliminate two panic paths in URI handling

### 🚜 Refactor

- Replace manual accumulation loops with iterator chains
- Drop redundant filter in parse_shellcheck_directive

### ⚡ Performance

- Avoid heap allocation in is_variable_in_read_command
- Avoid Vec allocation in for_each node traversal
- Use HashSet for visited URI tracking in source resolution

### ⚙️ Miscellaneous Tasks

- Update file(s): CHANGELOG.md
- Format shellcheck directive
- Simplify CRATES.md description
- Improve readability of completions logic
- Update file(s): Cargo.lock
- Precompute lowercase query in fuzzy search
- Deduplicate point computation in word_at_point
- Bump version to 0.2.4

### ◀️ Revert

- Restore loop in get_local_symbol_from_child
## [0.2.3] - 2026-05-25

### 🐛 Bug Fixes

- *(bench)* Accurate methodology and updated benchmark numbers
- Use workspace_folders for workspace root resolution

### 📚 Documentation

- Move gif to top of README
- Slow down demo.gif by 2x
- Update README motivation
- Tighten benchmark SVG layout
- Remove stale latency methodology description from README
- Update benchmark numbers (100x startup, 10x memory)
- Document cold start methodology and conservative 1600 ms figure
- Replace em dash with parens in benchmark note
- Add editor configs, limitations, and changelog
- Add AGENT.md pointing to CLAUDE.md
- Update CLAUDE.md to reflect refactored server/handlers structure
- Simplify path for zed
- Simplify explainshell explain
- Document conventional commits requirement in CONTRIBUTING.md

### ⚙️ Miscellaneous Tasks

- Exclude examples from published crate
- Remove redundant white space char in comment
- *(bench)* Remove latency measurement
- Add AGENT.md file to Cargo ignore
- Move get-options.sh into dedicated scripts directory
- Bump version to 0.2.3
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
