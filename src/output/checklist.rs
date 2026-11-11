use crate::scanner::traversal::{Classification, ScannedItem};

fn is_backupable(item: &ScannedItem) -> bool {
    item.path_is_utf8 && (item.is_dir || item.is_file || item.is_symlink)
}

fn inline_code(value: &str) -> String {
    let mut display = String::new();
    for character in value.chars() {
        match character {
            '\n' => display.push_str("\\n"),
            '\r' => display.push_str("\\r"),
            '\t' => display.push_str("\\t"),
            character if character.is_control() => {
                display.push_str(&format!("\\u{{{:x}}}", character as u32));
            }
            character => display.push(character),
        }
    }

    let mut max_backtick_run = 0;
    let mut current_backtick_run = 0;
    for character in display.chars() {
        if character == '`' {
            current_backtick_run += 1;
            max_backtick_run = max_backtick_run.max(current_backtick_run);
        } else {
            current_backtick_run = 0;
        }
    }

    let fence = "`".repeat(max_backtick_run + 1);
    format!("{fence} {display} {fence}")
}

/// Generates a Markdown checklist for restoration on the new machine.
pub fn generate_restore_checklist(items: &[ScannedItem]) -> String {
    let mut markdown = String::new();
    markdown.push_str("# nix-migrate Restoration Checklist\n\n");
    markdown.push_str("This checklist helps you restore your configurations, data, and secrets on your new machine.\n\n");

    markdown.push_str("## Summary of Backed-Up Items\n\n");

    let total = items.len();
    let managed = items
        .iter()
        .filter(|i| i.classification == Classification::Managed)
        .count();
    let cache = items
        .iter()
        .filter(|i| i.classification == Classification::Cache)
        .count();
    let config = items
        .iter()
        .filter(|i| i.classification == Classification::Config && is_backupable(i))
        .count();
    let data = items
        .iter()
        .filter(|i| i.classification == Classification::Data && is_backupable(i))
        .count();
    let secrets = items
        .iter()
        .filter(|i| i.classification == Classification::Secrets && is_backupable(i))
        .count();
    let unknown = items
        .iter()
        .filter(|i| i.classification == Classification::Unknown && is_backupable(i))
        .count();
    let special = items
        .iter()
        .filter(|i| {
            i.classification != Classification::Managed
                && i.classification != Classification::Cache
                && !is_backupable(i)
        })
        .count();

    markdown.push_str(&format!("- **Total items scanned**: {}\n", total));
    markdown.push_str(&format!("- **Nix-managed (skipped)**: {}\n", managed));
    markdown.push_str(&format!("- **Cache entries (skipped)**: {}\n", cache));
    markdown.push_str(&format!("- **Secrets to restore**: {}\n", secrets));
    markdown.push_str(&format!("- **Configurations to restore**: {}\n", config));
    markdown.push_str(&format!("- **User data to restore**: {}\n", data));
    markdown.push_str(&format!("- **Unclassified items**: {}\n", unknown));
    if special > 0 {
        markdown.push_str(&format!(
            "- **Special filesystem entries (not backed up)**: {}\n",
            special
        ));
    }
    markdown.push('\n');

    if secrets > 0 {
        markdown.push_str("## 🔐 Secrets Restoration Checklist\n\n");
        markdown.push_str("Secrets must be handled manually with extreme care.\n\n");
        for item in items
            .iter()
            .filter(|i| i.classification == Classification::Secrets && is_backupable(i))
        {
            markdown.push_str(&format!(
                "- [ ] Restore {} to original location\n",
                inline_code(&item.path)
            ));
            if let Some(desc) = &item.description {
                markdown.push_str(&format!("  * Reason: {}\n", inline_code(desc)));
            }
        }
        markdown.push('\n');
    }

    if config > 0 {
        markdown.push_str("## ⚙️ Configuration Restoration Checklist\n\n");
        markdown.push_str("Verify which configurations are now managed by Nix/home-manager before copying these files back.\n\n");
        for item in items
            .iter()
            .filter(|i| i.classification == Classification::Config && is_backupable(i))
        {
            markdown.push_str(&format!("- [ ] Restore {}\n", inline_code(&item.path)));
            if let Some(desc) = &item.description {
                markdown.push_str(&format!("  * Reason: {}\n", inline_code(desc)));
            }
        }
        markdown.push('\n');
    }

    if data > 0 {
        markdown.push_str("## 📁 User Data Restoration Checklist\n\n");
        markdown.push_str("Restore state/share data for applications to avoid losing local application state.\n\n");
        for item in items
            .iter()
            .filter(|i| i.classification == Classification::Data && is_backupable(i))
        {
            markdown.push_str(&format!("- [ ] Restore {}\n", inline_code(&item.path)));
            if let Some(desc) = &item.description {
                markdown.push_str(&format!("  * Reason: {}\n", inline_code(desc)));
            }
        }
        markdown.push('\n');
    }

    if unknown > 0 {
        markdown.push_str("## ❓ Unclassified Items Restoration Checklist\n\n");
        markdown.push_str(
            "These items could not be automatically identified. Please inspect them manually.\n\n",
        );
        for item in items
            .iter()
            .filter(|i| i.classification == Classification::Unknown && is_backupable(i))
        {
            markdown.push_str(&format!(
                "- [ ] Inspect and restore {} if needed\n",
                inline_code(&item.path)
            ));
            if let Some(desc) = &item.description {
                markdown.push_str(&format!("  * Reason: {}\n", inline_code(desc)));
            }
        }
        markdown.push('\n');
    }

    markdown.push_str("## 🚀 Restoration Best Practices\n\n");
    markdown.push_str("1. **First boots**: Set up your NixOS/home-manager configuration *before* restoring manually-backed-up dotfiles.\n");
    markdown.push_str("2. **Avoid clobbering**: If home-manager has already created a symlink for a file, DO NOT overwrite it. Instead, integrate the settings into your Nix configuration.\n");
    markdown.push_str("3. **Permissions**: Ensure private keys (e.g. in `.ssh`) have correct permissions (`600` or `700`) after restoring.\n");

    markdown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_restore_checklist() {
        let items = vec![
            ScannedItem {
                path: "/home/user/.config/app.toml".to_string(),
                path_is_utf8: true,
                relative_path: ".config/app.toml".to_string(),
                target: None,
                classification: Classification::Config,
                is_dir: false,
                is_file: true,
                is_symlink: false,
                size: 120,
                description: Some("Config file".to_string()),
            },
            ScannedItem {
                path: "/home/user/.ssh/id_rsa".to_string(),
                path_is_utf8: true,
                relative_path: ".ssh/id_rsa".to_string(),
                target: None,
                classification: Classification::Secrets,
                is_dir: false,
                is_file: true,
                is_symlink: false,
                size: 1600,
                description: Some("Secrets file".to_string()),
            },
        ];

        let checklist = generate_restore_checklist(&items);
        assert!(checklist.contains("# nix-migrate Restoration Checklist"));
        assert!(checklist.contains("Secrets Restoration Checklist"));
        assert!(checklist.contains(".config/app.toml"));
        assert!(checklist.contains(".ssh/id_rsa"));
    }

    #[test]
    fn checklist_escapes_control_characters_and_backticks() {
        let item = ScannedItem {
            path: "/home/user/`file\n- [x] forged".to_string(),
            path_is_utf8: true,
            relative_path: "`file\n- [x] forged".to_string(),
            target: None,
            classification: Classification::Unknown,
            is_dir: false,
            is_file: true,
            is_symlink: false,
            size: 0,
            description: Some("reason\n# forged heading".to_string()),
        };

        let checklist = generate_restore_checklist(&[item]);
        assert!(checklist.contains("\\n- [x] forged"));
        assert!(checklist.contains("\\n# forged heading"));
        assert!(!checklist.contains("\n- [x] forged"));
        assert!(!checklist.contains("\n# forged heading"));
    }

    #[test]
    fn special_files_are_reported_but_not_suggested_for_restore() {
        let special = ScannedItem {
            path: "/tmp/example-socket".to_string(),
            path_is_utf8: true,
            relative_path: "example-socket".to_string(),
            target: None,
            classification: Classification::Unknown,
            is_dir: false,
            is_file: false,
            is_symlink: false,
            size: 0,
            description: Some("Socket".to_string()),
        };
        let checklist = generate_restore_checklist(&[special]);

        assert!(checklist.contains("Special filesystem entries (not backed up)**: 1"));
        assert!(!checklist.contains("example-socket"));
    }
}
