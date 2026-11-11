#!/usr/bin/env bash
# Generic reference example only; use nix-migrate for a current scan.
# The example paths are placeholders. Customize and review them before use.

set -euo pipefail
umask 077

BACKUP_DIR="${1:-$HOME/migration-backup-$(date +%Y%m%d-%H%M%S)}"
case "$BACKUP_DIR" in
    /*) ;;
    *) BACKUP_DIR="$PWD/$BACKUP_DIR" ;;
esac

if [ -e "$BACKUP_DIR" ] || [ -L "$BACKUP_DIR" ]; then
    printf 'Refusing to overwrite existing destination: %s\n' "$BACKUP_DIR" >&2
    exit 1
fi

mkdir -p "$BACKUP_DIR"
printf 'Creating private backup in %s\n' "$BACKUP_DIR"

collect() {
    local label="$1" source="$2" destination="$3"
    local target="$BACKUP_DIR/$destination"

    if [ -e "$source" ] || [ -L "$source" ]; then
        mkdir -p "${target%/*}"
        cp -P -R "$source" "$target"
        printf '  [OK] %s\n' "$label"
    else
        printf '  [--] %s (not found, skipping)\n' "$label"
    fi
}

# Replace these placeholders with specific, reviewed, non-sensitive paths.
collect "Example application settings" \
    "$HOME/.config/example-app" \
    "config/example-app"
collect "Example application data" \
    "$HOME/.local/share/example-app" \
    "data/example-app"

cat > "$BACKUP_DIR/RESTORE-CHECKLIST.md" <<'CHECKLIST'
# Generic Migration Restore Checklist

- [ ] Review the backup contents before restoring them.
- [ ] Install and configure applications on the new machine first.
- [ ] Restore only the settings and data you still need.
- [ ] Handle private keys, credentials, and tokens through an appropriate
      encrypted, application-specific process; this example does not collect them.
- [ ] Encrypt the backup before transferring or storing it elsewhere.
CHECKLIST

printf 'Backup complete: %s\n' "$BACKUP_DIR"
printf '%s\n' 'Review the checklist and encrypt the backup before transferring it.'
