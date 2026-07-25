use crate::scanner::traversal::{ScannedItem, Classification};

/// Generates a backup bash script from a list of scanned items.
pub fn generate_backup_script(items: &[ScannedItem]) -> String {
    let mut script = String::new();
    script.push_str("#!/usr/bin/env bash\n");
    script.push_str("# nix-migrate auto-generated backup script\n");
    script.push_str("# Usage: ./backup.sh [destination_directory]\n\n");
    script.push_str("set -euo pipefail\n\n");
    script.push_str("DEST_DIR=\"${1:-$HOME/nix_migrate_backup}\"\n");
    script.push_str("echo \"==> Starting backup to: $DEST_DIR\"\n");
    script.push_str("mkdir -p \"$DEST_DIR\"\n\n");

    // We filter out Managed (Nix-managed) and Cache files, as they don't need backup.
    let backup_items: Vec<&ScannedItem> = items
        .iter()
        .filter(|item| {
            item.classification != Classification::Managed && item.classification != Classification::Cache
        })
        .collect();

    if backup_items.is_empty() {
        script.push_str("echo \"No unmanaged configurations, data or secrets found to back up!\"\n");
        return script;
    }

    script.push_str("# --- Backup Secrets ---\n");
    let secrets: Vec<&&ScannedItem> = backup_items.iter().filter(|i| i.classification == Classification::Secrets).collect();
    if secrets.is_empty() {
        script.push_str("echo \"No secrets found.\"\n");
    } else {
        script.push_str("echo \"Copying secrets...\"\n");
        for item in secrets {
            script.push_str(&format!(
                "mkdir -p \"$DEST_DIR/secrets\"\ncp -r --parents \"{}\" \"$DEST_DIR/secrets/\" 2>/dev/null || true\n",
                item.path
            ));
        }
    }
    script.push('\n');

    script.push_str("# --- Backup Configurations ---\n");
    let configs: Vec<&&ScannedItem> = backup_items.iter().filter(|i| i.classification == Classification::Config).collect();
    if configs.is_empty() {
        script.push_str("echo \"No configurations found.\"\n");
    } else {
        script.push_str("echo \"Copying configurations...\"\n");
        for item in configs {
            script.push_str(&format!(
                "mkdir -p \"$DEST_DIR/config\"\ncp -r --parents \"{}\" \"$DEST_DIR/config/\" 2>/dev/null || true\n",
                item.path
            ));
        }
    }
    script.push('\n');

    script.push_str("# --- Backup Data ---\n");
    let data: Vec<&&ScannedItem> = backup_items.iter().filter(|i| i.classification == Classification::Data).collect();
    if data.is_empty() {
        script.push_str("echo \"No data files found.\"\n");
    } else {
        script.push_str("echo \"Copying user data...\"\n");
        for item in data {
            script.push_str(&format!(
                "mkdir -p \"$DEST_DIR/data\"\ncp -r --parents \"{}\" \"$DEST_DIR/data/\" 2>/dev/null || true\n",
                item.path
            ));
        }
    }
    script.push('\n');

    script.push_str("# --- Backup Unknown/Unclassified ---\n");
    let unknown: Vec<&&ScannedItem> = backup_items.iter().filter(|i| i.classification == Classification::Unknown).collect();
    if !unknown.is_empty() {
        script.push_str("echo \"Copying unknown/unclassified files...\"\n");
        for item in unknown {
            script.push_str(&format!(
                "mkdir -p \"$DEST_DIR/unknown\"\ncp -r --parents \"{}\" \"$DEST_DIR/unknown/\" 2>/dev/null || true\n",
                item.path
            ));
        }
    }
    script.push('\n');

    script.push_str("echo \"==> Backup completed successfully!\"\n");
    script
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_backup_script() {
        let items = vec![
            ScannedItem {
                path: "/home/user/.config/app.toml".to_string(),
                target: None,
                classification: Classification::Config,
                is_dir: false,
                size: 120,
                description: Some("Config file".to_string()),
            },
            ScannedItem {
                path: "/home/user/.ssh/id_rsa".to_string(),
                target: None,
                classification: Classification::Secrets,
                is_dir: false,
                size: 1600,
                description: Some("Secrets file".to_string()),
            },
            ScannedItem {
                path: "/home/user/.cache/temporary".to_string(),
                target: None,
                classification: Classification::Cache,
                is_dir: false,
                size: 9999,
                description: Some("Cache file".to_string()),
            },
        ];

        let script = generate_backup_script(&items);
        assert!(script.contains("/secrets"));
        assert!(script.contains("/config"));
        assert!(script.contains(".config/app.toml"));
        assert!(script.contains(".ssh/id_rsa"));
        // Cache files should be excluded
        assert!(!script.contains("temporary"));
    }
}
