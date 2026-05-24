# Contributing

## Reporting bugs

Open a GitHub issue. Include:

- What you did, what you expected, what happened
- OS and editor
- `bashls` version (`bashls --version`)
- Relevant log output if available

## Pull requests

1. Fork the repo and create a branch
2. Make your changes — run `just ci` before submitting
3. Rebase onto `main` before opening a PR (merge commits are not allowed)
4. Open a PR with a clear description of what and why
4. If any commit was written with AI assistance, include a `Co-Authored-By` trailer identifying the model, e.g.:
   ```
   Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
   ```

## Build

```
just build   # cargo build --release
just ci      # fmt check, lint, audit, test
```

See `CLAUDE.md` for architecture notes.

## License

By contributing you agree your changes will be licensed under the same [MIT license](LICENSE) as this project.
