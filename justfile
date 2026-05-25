default:
    @just --list

# compile release binary
build:
    cargo build --release

# build and install release binary to ~/.cargo/bin
install: build
    cargo install --path . --offline

# check for compile errors without building
check:
    cargo check

# run clippy lints
lint:
    cargo clippy -- -D warnings

# run clippy with pedantic and nursery lints (informational — some noise expected)
lint-strict:
    cargo clippy -- -D warnings -W clippy::pedantic -W clippy::nursery

# format source code
fmt:
    cargo fmt

# check formatting without modifying files
fmt-check:
    cargo fmt -- --check

# run tests
test:
    cargo test

# vulnerability audit
audit:
    cargo audit

# remove build artifacts
clean:
    cargo clean

# update dependencies
update:
    cargo update

# run all CI checks
ci: fmt-check lint audit test

# benchmark startup time and memory
bench:
    @echo "=== Startup time ==="
    hyperfine --warmup 3 -i 'echo "{}" | ./target/release/bashls start'
    @echo "=== Memory (RSS) ==="
    /usr/bin/time -v bash -c 'echo "{}" | ./target/release/bashls start' 2>&1 | grep "Maximum resident"

# release: bump version, commit, tag, and push
release version: ci
    cargo set-version {{version}}
    git add Cargo.toml Cargo.lock
    git commit -m "chore: bump version to {{version}}"
    git tag v{{version}} -m "v{{version}}"
    git push origin main --tags

# publish to crates.io
publish: ci build
    cargo publish

# verify build with a neovim test config
nvim:
    cargo build
    nvim -u contrib/init.lua contrib/example.sh
