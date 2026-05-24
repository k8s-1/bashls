# Contributing

## Reporting bugs

Open a GitHub issue. Include:

- What you did, what you expected, what happened
- OS and editor
- `bls` version (`bls --version`)
- Relevant log output if available

## Pull requests

1. Fork the repo and create a branch
2. Make your changes — run `just test` and `just lint` before submitting
3. Open a PR with a clear description of what and why
4. If any commit was written with AI assistance, include a `Co-Authored-By` trailer identifying the model, e.g.:
   ```
   Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
   ```

## Build

```
just build   # cargo build --release
just test    # cargo test
just lint    # cargo clippy -- -D warnings
just fmt     # cargo fmt
```

See `CLAUDE.md` for architecture notes.

## License

By contributing you agree your changes will be licensed under the same [MIT license](LICENSE) as this project.
