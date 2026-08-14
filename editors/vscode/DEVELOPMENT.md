# Development

## Build

```
just build
```

Type-checks with `tsc` then bundles `src/extension.ts` into `out/extension.js` with `bun build`. No runtime `node_modules` are shipped — dependencies are inlined into the bundle.

## Package locally

```
just package
code --install-extension bashls.vsix
```

## Publishing

Not published to any marketplace. CI builds `bashls.vsix` and attaches it to the GitHub Release when a `bashls-vscode@*` tag is pushed (`.github/workflows/vscode-release.yml`). Users install it with `code --install-extension bashls.vsix` — works for VS Code, VSCodium, Cursor, Windsurf, and other VS Code forks.

Tag prefix is `bashls-vscode@*`, not `v*` or `vscode-v*` — the CLI's own release workflow triggers on `tags: ['v*']`, and GitHub Actions tag globs match on prefix regardless of what follows, so a tag like `vscode-v0.1.0` would also match `v*` and fire both release workflows off the same tag. `bashls-vscode@*` avoids the collision (npm/lerna-style monorepo package tagging).

(Not on the VS Code Marketplace — that requires an Azure DevOps organization, which now requires a credit card just to create, on top of a PAT type Microsoft is retiring December 2026. Not on Open VSX either — its signup flow requires a separate Eclipse Foundation account, which had a broken email verification step. Revisit either if circumstances change.)

## Releasing

```
just release          # patch bump (default)
just release minor
just release major
```

Bumps `version` in `package.json`, commits, tags `bashls-vscode@<new version>`, and pushes — CI then builds and attaches `bashls.vsix` to the GitHub Release.
