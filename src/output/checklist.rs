use crate::scanner::traversal::{ScannedItem, Classification};

/// Generates a Markdown checklist for restoration on the new machine.
pub fn generate_restore_checklist(items: &[ScannedItem]) -> String {
    let mut markdown = String::new();
    markdown.push_str("# nix-migrate Restoration Checklist\n\n");
    markdown.push_str("This checklist helps you restore your configurations, data, and secrets on your new machine.\n\n");

    markdown.push_str("## Summary of Backed-Up Items\n\n");

    let total = items.len();
    let managed = items.iter().filter(|i| i.classification == Classification::Managed).count();
    let cache = items.iter().filter(|i| i.classification == Classification::Cache).count();
    let config = items.iter().filter(|i| i.classification == Classification::Config).count();
    let data = items.iter().filter(|i| i.classification == Classification::Data).count();
    let secrets = items.iter().filter(|i| i.classification == Classification::Secrets).count();
    let unknown = items.iter().filter(|i| i.classification == Classification::Unknown).count();

    markdown.push_str(&format!("- **Total items scanned**: {}\n", total));
    markdown.push_str(&format!("- **Nix-managed (skipped)**: {}\n", managed));
    markdown.push_str(&format!("- **Cache files (skipped)**: {}\n", cache));
    markdown.push_str(&format!("- **Secrets to restore**: {}\n", secrets));
    markdown.push_str(&format!("- **Configurations to restore**: {}\n", config));
    markdown.push_str(&format!("- **User data to restore**: {}\n", data));
    markdown.push_str(&format!("- **Unclassified items**: {}\n\n", unknown));

    if secrets > 0 {
        markdown.push_str("## 🔐 Secrets Restoration Checklist\n\n");
        markdown.push_str("Secrets must be handled manually with extreme care.\n\n");
        for item in items.iter().filter(|i| i.classification == Classification::Secrets) {
            markdown.push_str(&format!("- [ ] Restore `{}` to original location\n", item.path));
            if let Some(desc) = &item.description {
                markdown.push_str(&format!("  * Reason: {}\n", desc));
            }
        }
        markdown.push('\n');
    }

    if config > 0 {
        markdown.push_str("## ⚙️ Configuration Restoration Checklist\n\n");
        markdown.push_str("Verify which configurations are now managed by Nix/home-manager before copying these files back.\n\n");
        for item in items.iter().filter(|i| i.classification == Classification::Config) {
            markdown.push_str(&format!("- [ ] Restore `{}`\n", item.path));
            if let Some(desc) = &item.description {
                markdown.push_str(&format!("  * Reason: {}\n", desc));
            }
        }
        markdown.push('\n');
    }

    if data > 0 {
        markdown.push_str("## 📁 User Data Restoration Checklist\n\n");
        markdown.push_str("Restore state/share data for applications to avoid losing local application state.\n\n");
        for item in items.iter().filter(|i| i.classification == Classification::Data) {
            markdown.push_str(&format!("- [ ] Restore `{}`\n", item.path));
            if let Some(desc) = &item.description {
                markdown.push_str(&format!("  * Reason: {}\n", desc));
            }
        }
        markdown.push('\n');
    }

    if unknown > 0 {
        markdown.push_str("## ❓ Unclassified Items Restoration Checklist\n\n");
        markdown.push_str("These items could not be automatically identified. Please inspect them manually.\n\n");
        for item in items.iter().filter(|i| i.classification == Classification::Unknown) {
            markdown.push_str(&format!("- [ ] Inspect and restore `{}` if needed\n", item.path));
            if let Some(desc) = &item.description {
                markdown.push_str(&format!("  * Reason: {}\n", desc));
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
        ];

        let checklist = generate_restore_checklist(&items);
        assert!(checklist.contains("# nix-migrate Restoration Checklist"));
        assert!(checklist.contains("Secrets Restoration Checklist"));
        assert!(checklist.contains(".config/app.toml"));
        assert!(checklist.contains(".ssh/id_rsa"));
    }
}
