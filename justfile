default:
    @just --list

# compile release binary
build:
    cargo build --release

# check for compile errors without building
check:
    cargo check

# run clippy lints
lint:
    cargo clippy -- -D warnings

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
    hyperfine --warmup 3 -i 'echo "{}" | ./target/release/bls start'
    @echo "=== Memory (RSS) ==="
    /usr/bin/time -v bash -c 'echo "{}" | ./target/release/bls start' 2>&1 | grep "Maximum resident"

# publish to crates.io
publish: ci build
    cargo publish

# verify build with a neovim test config
nvim:
    cargo build
    nvim -u contrib/init.lua contrib/example.sh
