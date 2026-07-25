# nix-migrate

A high-performance, Nix-aware machine migration assistant built in Rust. It scans home directories, classifies unmanaged files, and generates targeted backup scripts and restoration checklists. It exposes its capabilities via the standard **Model Context Protocol (MCP)**, enabling AI agents (such as Claude or opencode) to reason about unfamiliar application files and automate the backup process.

---

## 🚀 The Problem

When migrating to a new machine, it is difficult to determine which files are **not** managed by your declarative Nix configuration (e.g. NixOS/home-manager) and must be backed up manually.
* Traditional backup tools (`mackup`, `rsync`) do not understand `/nix/store/` symlinks.
* Dotfile trackers (`chezmoi`) only list files they already track.
* Statically hardcoding known configuration directories creates brittle, high-maintenance databases.

`nix-migrate` solves this by introducing a **hybrid architecture**:
1. **The Fast Scanner (Rust Binary)**: Deterministically traverses the filesystem, filters ignores, recursively resolves symlinks to check if they point into `/nix/store/`, and categorizes unmanaged files based on high-performance heuristics.
2. **The Reasoning Layer (LLM/MCP Client)**: Uses the scanner's structured output via MCP to research unfamiliar application paths, ask about ambiguous credentials, and orchestrate the migration.

---

## 🛠️ Features

* **Nix-Aware Symlink Resolution**: Resolves symlinks recursively (using `std::fs::canonicalize`) to detect if they ultimately point into `/nix/store/`.
* **Early Recursion Pruning**: If a directory itself is symlinked to the Nix store, `nix-migrate` skips walking its children, preventing redundant scans of thousands of read-only store files.
* **Heuristic File Classification**: Automatically categorizes unmanaged paths into:
  - `Managed`: Pointing to Nix store.
  - `Config`: Configuration files (e.g. `.toml`, `.json`, `.yaml`, `.conf`, `rc` files).
  - `Data`: Local sqlite databases, application share files, documents.
  - `Cache`: Package manager registries, target builds, standard caches (skipped during backup).
  - `Secrets`: SSH private keys, credentials, tokens, `.env` files.
  - `Unknown`: Unclassified paths needing LLM/user review.
* **Robust Path Filtering (`.nix-migrate-ignore`)**: Standard ignore list supporting comments, empty lines, root-relative patterns (`/pattern`), and component-matching.
* **First-Class MCP Server**: Exposes filesystem scanning, backup script generation, and checklist generation as standard JSON-RPC tools over stdio.

---

## 📐 Hybrid Architecture

```
                                  MCP Protocol
┌─────────────────────────────┐                  ┌──────────────────────────────┐
│  Rust Binary (nix-migrate)  │ ◄──────────────► │  Any MCP Client              │
│  (Fast Scanner + Tools)     │                  │  (opencode, Claude Desktop,  │
│                             │                  │   VS Code, etc.)             │
│  Tools:                     │                  │                              │
│  - scan_home()              │                  │  • LLM reasoning layer       │
│  - generate_backup_script() │                  │  • Interactively asks user   │
│  - generate_restore_check() │                  │  • Safely executes scripts   │
└─────────────────────────────┘                  └──────────────────────────────┘
```

---

## 🎛️ Exposed MCP Tools

The server runs on standard input/output (stdio) and exposes three primary tools:

### 1. `scan_home`
Scans a target directory, filters out ignored paths, resolves symlinks, and returns a JSON list of categorized files and folders.
* **Parameters**:
  - `path` (string, optional): Directory to scan (defaults to current directory).
  - `ignore_file` (string, optional): Path to a custom ignore file.

### 2. `generate_backup_script`
Generates a portable bash script (`backup.sh`) designed to copy unmanaged configurations, secrets, and data safely into a specified destination.
* **Parameters**:
  - `items` (array of ScannedItem): The categorized list returned by `scan_home`.

### 3. `generate_restore_checklist`
Generates a beautifully structured Markdown restoration checklist, helping you restore each category on your new machine with safety tips (e.g. private key file permissions).
* **Parameters**:
  - `items` (array of ScannedItem): The categorized list returned by `scan_home`.

---

## 📦 Local Setup & Development

Because this project is built inside a **NixOS WSL** environment, development dependencies are declared in `flake.nix` and managed via **direnv**.

### 1. Install & Allow direnv
```bash
direnv allow
```

### 2. Check & Build
Always run commands wrapped in the Nix shell environment:
```bash
# Verify compilation
nix develop -c cargo check

# Run Clippy (warnings treated as errors)
nix develop -c cargo clippy -- -D warnings

# Execute test suite
nix develop -c cargo test

# Build optimized release binary
nix develop -c cargo build --release
```

---

## ⚙️ Configuration (`.nix-migrate-ignore`)

Place a `.nix-migrate-ignore` file in the root of the directory you want to scan to filter out large build folders, system files, or unneeded directories:

```ini
# Ignore standard package manager caches
node_modules
.cargo/registry
.cargo/git
.npm

# Ignore VCS
.git

# Ignore root-level folders only
/downloads
/videos

# Ignore specific caches
.cache
__pycache__
.pytest_cache
```

---

## 🔌 MCP Client Configuration

To integrate `nix-migrate` into Claude Desktop, add the following to your configuration file (typically `~/.config/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "nix-migrate": {
      "command": "nix",
      "args": [
        "develop",
        "/path/to/nix-migrate",
        "-c",
        "cargo",
        "run",
        "--release"
      ]
    }
  }
}
```

---

## 📄 License

This project is licensed under the MIT License.
