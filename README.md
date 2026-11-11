# nix-migrate

`nix-migrate` is a Nix-aware filesystem scanner and local Model Context Protocol (MCP) server. It helps you find files that may need manual migration, then generates a backup script and a restore checklist for you to review.

The scanner is deterministic and does not use an embedded LLM. Any reasoning happens in the MCP client.

## Features

- Scans a directory tree without following directory symlinks.
- Resolves symlink targets to identify paths under `/nix/store` as `Managed`.
- Applies `.nix-migrate-ignore` patterns before classification.
- Classifies scan results as `Managed`, `Config`, `Data`, `Cache`, `Secrets`, or `Unknown`.
- Exposes scan, backup-script, and restore-checklist tools over stdio MCP.
- Generates scripts that preserve relative paths, avoid shell interpolation of file names, use a private umask, skip special filesystem entries, and stop on copy errors.

Classification is heuristic: review results before relying on them for a migration.

| Classification | Meaning |
| --- | --- |
| `Managed` | Resolves into `/nix/store`; excluded from backups. |
| `Config` | Likely application or shell configuration. |
| `Data` | Likely user files or local application state. |
| `Cache` | Likely temporary or regenerable data; excluded from backups. |
| `Secrets` | Likely credentials, tokens, or key material. |
| `Unknown` | Needs manual review. |

## Safety and privacy

- The scanner reads directory entries and metadata; it does not read the contents of scanned files. It does read the selected ignore file.
- Scan results include file paths and symlink targets. Only scan directories whose names and structure you are comfortable sharing with your MCP client.
- The server runs as a local stdio process, with the permissions of the user who starts it. A client can request a scan of any directory that user can access.
- The generated backup script is not executed by the server. Review it before running. It excludes `Managed` and `Cache` entries, does not recursively copy directory entries, and preserves symlinks rather than following them.
- The script creates a new destination with `umask 077` and refuses to overwrite an existing destination. Choose a destination that does not already exist. Backups can still contain private keys, tokens, or personal data: encrypt them before transferring or storing them elsewhere.

## MCP tools

### `scan_home`

Scans a directory and returns its classified items.

| Parameter | Required | Description |
| --- | --- | --- |
| `path` | No | Directory to scan; defaults to the server's current working directory. |
| `ignore_file` | No | Path to a custom `.nix-migrate-ignore` file. By default, the scanner looks for that file in the scan root. |

Each item includes its absolute path, path relative to the scan root, classification, size, file-type flags, a UTF-8 path indicator (`path_is_utf8`), and a symlink target when applicable. Non-UTF-8 paths are represented lossily for reporting and are not copied by generated scripts.

### `generate_backup_script`

Accepts the `items` returned by `scan_home` and returns a Bash script. The script backs up `Config`, `Data`, `Secrets`, and `Unknown` items. It does not back up `Managed` or `Cache` items.

### `generate_restore_checklist`

Accepts the same `items` and returns a Markdown checklist. File names are rendered as code so unusual names cannot introduce extra checklist content.

## Ignore file

Create `.nix-migrate-ignore` in the scan root to skip entries. Blank lines and lines beginning with `#` are ignored. A plain name matches that path component anywhere in the tree; a leading `/` makes a pattern relative to the scan root. Wildcards are not supported.

```text
# Match these components anywhere
node_modules
.cache
__pycache__

# Match only these paths from the scan root
/Downloads
/Videos
```

## Run and develop

The Nix flake provides the application package and a development shell. From the repository root:

```bash
# Start the stdio MCP server
nix run .

# Enter the development environment
nix develop

# Validate changes
nix develop -c cargo fmt --check
nix develop -c cargo check --locked
nix develop -c cargo clippy --locked -- -D warnings
nix develop -c cargo test --locked

# Build the Nix package
nix build
```

The server waits for MCP JSON-RPC messages on standard input and writes responses to standard output; it is not an interactive command-line scanner.

## OpenCode and other MCP clients

Register `nix-migrate` as a **local stdio MCP server** in your client. The command to launch the flake from another working directory is:

```text
nix run /absolute/path/to/nix-migrate
```

Replace the example path with the repository's absolute path. MCP configuration formats vary by client and version; use the current configuration reference for your client rather than copying an outdated config schema.

## Project layout

- `src/scanner/` — ignore parsing, symlink checks, traversal, and classification.
- `src/mcp/` — stdio MCP server and tool definitions.
- `src/output/` — backup-script and restore-checklist generation.
- `ARCHITECTURE.md` — implementation and trust-boundary notes.

## License

MIT; see [LICENSE](LICENSE).
