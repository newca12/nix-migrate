# Contributor and agent notes

## Development environment

The repository's Nix flake provides the Rust toolchain. Wrap build, lint, and test commands with `nix develop`:

```bash
nix develop -c cargo fmt --check
nix develop -c cargo check --locked
nix develop -c cargo clippy --locked -- -D warnings
nix develop -c cargo test --locked
```

## Architecture and safety

- The Rust binary is a deterministic scanner and local stdio MCP server; do not add an embedded LLM client.
- Keep the modern module layout (`src/scanner.rs` with `src/scanner/*.rs`, rather than `mod.rs`).
- The scanner must not follow symlinked directories. Resolve symlinks to detect Nix-store targets and prune managed entries.
- Keep classification values synchronized with the public categories: `Managed`, `Config`, `Data`, `Cache`, `Secrets`, and `Unknown`.
- Treat paths and descriptions returned by the filesystem as untrusted input. Quote shell arguments, escape generated Markdown, and never execute generated scripts from the MCP server.
- Keep backup generation private by default, do not hide copy errors, and avoid copying excluded cache contents through parent directories.
- Keep dependencies focused. `schemars` must remain compatible with the version required by `rmcp`.

## Project checks

Before submitting changes, run formatting, compilation, Clippy, and tests using the commands above. Update the README and `ARCHITECTURE.md` when tool behavior or safety guarantees change.
