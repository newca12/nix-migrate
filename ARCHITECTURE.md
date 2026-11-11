# Architecture and security model

## Components

- `src/scanner/` traverses a selected directory, applies ignore rules, resolves symlinks, and classifies entries.
- `src/mcp/` exposes the scanner and output generators as tools over a local stdio MCP connection.
- `src/output/` generates a Bash backup script and a Markdown restore checklist. Neither output is executed by the server.

There is no network listener or embedded model client. The MCP client is responsible for any research or reasoning about unknown files.

## Scan behavior

The requested root is resolved to an absolute directory before scanning. `walkdir` does not follow directory symlinks. Symlink targets are canonicalized when possible; targets under `/nix/store` are classified as managed. A dangling absolute symlink into `/nix/store` is also treated as managed, while a substring in an unrelated path is not.

The scanner reports paths and metadata, not scanned file contents. It reads the configured ignore file. Paths are returned as strings; entries whose paths are not valid UTF-8 are marked and are not copied by generated outputs. Ignore patterns are literal path/component matches, not glob expressions. If traversal encounters an error, the scan fails instead of silently reporting an incomplete result.

Classification is heuristic and ordered from sensitive/cache-specific cases to broader configuration and data cases. It can produce false positives and false negatives; users should review the result.

## Generated outputs

The backup script excludes `Managed` and `Cache` entries. It creates directory entries without recursively copying their contents, so an excluded child is not reintroduced by copying a parent. It copies regular files and preserves symlinks without following them; special entries and non-UTF-8 paths are skipped. It keeps paths relative to the selected scan root, quotes shell arguments, uses `umask 077`, refuses an existing destination, and stops on copy errors or an unexpected source type.

The backup produced by running the script may contain private keys, credentials, personal documents, or other sensitive data. The script is generated as text and is never executed by the server. Review it before running and encrypt the resulting backup before transferring or storing it elsewhere. The restore checklist treats file names and descriptions as untrusted text.

## Trust boundary

The server runs with the operating-system permissions of its user. An MCP client can request a scan of any directory that user can access, and receives the returned paths, metadata, and symlink targets. Run it only with clients you trust and only scan directories whose names and structure you are willing to disclose to that client.

The backup generator consumes items supplied by the MCP caller. It safely quotes paths and refuses invalid path shapes, but generated scripts should still be reviewed as user-controlled output before execution.
