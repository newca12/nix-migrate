use crate::scanner::traversal::{Classification, ScannedItem};
use std::path::{Component, Path};

/// Quote a string as one POSIX shell argument.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn backup_relative_path(item: &ScannedItem) -> Option<String> {
    if item.relative_path.contains('\0') {
        return None;
    }

    let (path, from_source_path) = if item.relative_path.is_empty() {
        (Path::new(&item.path), true)
    } else {
        (Path::new(&item.relative_path), false)
    };

    if from_source_path != path.is_absolute() {
        return None;
    }

    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Prefix(_) if from_source_path => {}
            Component::Normal(component) => components.push(component.to_str()?.to_owned()),
            Component::CurDir => {}
            _ => return None,
        }
    }

    if components.is_empty() {
        None
    } else {
        Some(components.join("/"))
    }
}

fn valid_source_path(path: &str) -> bool {
    let source = Path::new(path);
    source.is_absolute()
        && !path.contains('\0')
        && source
            .components()
            .all(|component| !matches!(component, Component::ParentDir))
}

/// Generates a bash backup script from a list of scanned items.
pub fn generate_backup_script(items: &[ScannedItem]) -> String {
    let backup_items: Vec<&ScannedItem> = items
        .iter()
        .filter(|item| {
            item.classification != Classification::Managed
                && item.classification != Classification::Cache
                && item.path_is_utf8
                && (item.is_dir || item.is_file || item.is_symlink)
        })
        .collect();
    let skipped_unsupported_entries = items.iter().any(|item| {
        item.classification != Classification::Managed
            && item.classification != Classification::Cache
            && (!item.path_is_utf8 || (!item.is_dir && !item.is_file && !item.is_symlink))
    });

    let has_invalid_path = backup_items
        .iter()
        .any(|item| !valid_source_path(&item.path) || backup_relative_path(item).is_none());

    let mut script = String::new();
    script.push_str("#!/usr/bin/env bash\n");
    script.push_str("# nix-migrate generated backup script\n");
    script
        .push_str("# Review the selected files before use; backups may contain sensitive data.\n");
    script.push_str("# Usage: ./backup.sh [new_destination_directory]\n\n");
    script.push_str("set -euo pipefail\n\n");

    if has_invalid_path {
        script.push_str(
            "printf '%s\\n' 'Refusing to generate a backup from invalid scan paths.' >&2\nexit 1\n",
        );
        return script;
    }

    if backup_items.is_empty() {
        script.push_str(
            "printf '%s\\n' 'No backupable unmanaged configurations, data, or secrets found.'\n",
        );
        if skipped_unsupported_entries {
            script.push_str(
                "printf '%s\\n' 'Unsupported or non-UTF-8 filesystem entries were skipped.' >&2\n",
            );
        }
        return script;
    }

    script.push_str("DEST_DIR=\"${1:-$HOME/nix_migrate_backup}\"\n");
    script.push_str("case \"$DEST_DIR\" in\n  /*) ;;\n  *) DEST_DIR=\"$PWD/$DEST_DIR\" ;;\nesac\n");
    script.push_str("umask 077\n");
    script.push_str(
        "if [ -e \"$DEST_DIR\" ] || [ -L \"$DEST_DIR\" ]; then\n  printf 'Refusing to overwrite existing destination: %s\\n' \"$DEST_DIR\" >&2\n  exit 1\nfi\n",
    );
    script.push_str(
        "if ! mkdir -p \"$DEST_DIR\"; then\n  printf 'Could not create destination: %s\\n' \"$DEST_DIR\" >&2\n  exit 1\nfi\n",
    );
    script.push_str("printf '==> Starting private backup in: %s\\n' \"$DEST_DIR\"\n\n");
    script.push_str(
        "backup_directory() {\n  local category=\"$1\" source=\"$2\" relative=\"$3\"\n  if [ ! -d \"$source\" ] || [ -L \"$source\" ]; then\n    printf 'Refusing to create backup directory for an unexpected source: %s\\n' \"$source\" >&2\n    return 1\n  fi\n  mkdir -p \"$DEST_DIR/$category/$relative\"\n}\n\n",
    );
    script.push_str(
        "backup_item() {\n  local category=\"$1\" source=\"$2\" relative=\"$3\"\n  local destination=\"$DEST_DIR/$category/$relative\"\n  local parent=\"${destination%/*}\"\n  mkdir -p \"$parent\"\n  if [ -e \"$destination\" ] || [ -L \"$destination\" ]; then\n    printf 'Refusing to overwrite backup item: %s\\n' \"$relative\" >&2\n    return 1\n  fi\n  if [ ! -f \"$source\" ] && [ ! -L \"$source\" ]; then\n    printf 'Refusing to copy a non-regular filesystem entry: %s\\n' \"$source\" >&2\n    return 1\n  fi\n  cp -P -R \"$source\" \"$destination\"\n}\n\n",
    );

    let categories = [
        (Classification::Secrets, "secrets", "Secrets"),
        (Classification::Config, "config", "Configurations"),
        (Classification::Data, "data", "Data"),
        (Classification::Unknown, "unknown", "Unclassified items"),
    ];

    for (classification, directory, label) in categories {
        let category_items: Vec<&ScannedItem> = backup_items
            .iter()
            .copied()
            .filter(|item| item.classification == classification)
            .collect();

        script.push_str(&format!("# --- Backup {label} ---\n"));
        if category_items.is_empty() {
            let lowercase_label = label.to_lowercase();
            script.push_str(&format!(
                "printf '%s\\n' {}\n\n",
                shell_quote(&format!("No {lowercase_label} found."))
            ));
            continue;
        }

        let lowercase_label = label.to_lowercase();
        script.push_str(&format!(
            "printf '%s\\n' {}\n",
            shell_quote(&format!("Copying {lowercase_label}..."))
        ));
        for item in category_items {
            let relative = backup_relative_path(item)
                .expect("paths were checked before the script was generated");
            if item.is_dir && !item.is_symlink {
                script.push_str(&format!(
                    "backup_directory {} {} {}\n",
                    shell_quote(directory),
                    shell_quote(&item.path),
                    shell_quote(&relative)
                ));
            } else {
                script.push_str(&format!(
                    "backup_item {} {} {}\n",
                    shell_quote(directory),
                    shell_quote(&item.path),
                    shell_quote(&relative)
                ));
            }
        }
        script.push('\n');
    }

    if skipped_unsupported_entries {
        script.push_str(
            "printf '%s\\n' 'Skipped unsupported or non-UTF-8 filesystem entries.' >&2\n",
        );
    }

    script.push_str("printf '%s\\n' '==> Backup completed successfully.'\n");
    script.push_str(
        "printf '%s\\n' 'Encrypt the backup before transferring or storing it elsewhere.'\n",
    );
    script
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::process::{Command, Stdio};

    fn item(
        path: &str,
        relative_path: &str,
        classification: Classification,
        is_dir: bool,
        is_symlink: bool,
    ) -> ScannedItem {
        ScannedItem {
            path: path.to_string(),
            path_is_utf8: true,
            relative_path: relative_path.to_string(),
            target: None,
            classification,
            is_dir,
            is_file: !is_dir && !is_symlink,
            is_symlink,
            size: 0,
            description: None,
        }
    }

    #[test]
    fn generated_script_uses_private_destination_and_skips_caches() {
        let items = vec![
            item(
                "/home/user/.config/app.toml",
                ".config/app.toml",
                Classification::Config,
                false,
                false,
            ),
            item(
                "/home/user/.ssh/id_ed25519",
                ".ssh/id_ed25519",
                Classification::Secrets,
                false,
                false,
            ),
            item(
                "/home/user/.cache/temp",
                ".cache/temp",
                Classification::Cache,
                false,
                false,
            ),
        ];

        let script = generate_backup_script(&items);
        assert!(script.contains("umask 077"));
        assert!(script.contains("cp -P -R"));
        assert!(script.contains(".config/app.toml"));
        assert!(script.contains(".ssh/id_ed25519"));
        assert!(!script.contains(".cache/temp"));
        assert!(!script.contains("|| true"));
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(
            shell_quote("a'; touch /tmp/pwn #"),
            "'a'\"'\"'; touch /tmp/pwn #'"
        );
    }

    #[test]
    fn directories_are_created_without_recursively_copying_excluded_children() {
        let items = vec![
            item(
                "/home/user/.local/share",
                ".local/share",
                Classification::Data,
                true,
                false,
            ),
            item(
                "/home/user/.local/share/app/cache",
                ".local/share/app/cache",
                Classification::Cache,
                true,
                false,
            ),
            item(
                "/home/user/.local/share/app/settings.db",
                ".local/share/app/settings.db",
                Classification::Data,
                false,
                false,
            ),
        ];

        let script = generate_backup_script(&items);
        assert!(
            script.contains("backup_directory 'data' '/home/user/.local/share' '.local/share'")
        );
        assert!(script.contains("backup_item 'data' '/home/user/.local/share/app/settings.db'"));
        assert!(!script.contains("/home/user/.local/share/app/cache"));
    }

    #[test]
    fn unsafe_paths_refuse_to_generate_a_backup() {
        let items = vec![item(
            "/home/user/.config/app.toml",
            "../outside",
            Classification::Config,
            false,
            false,
        )];

        let script = generate_backup_script(&items);
        assert!(script.contains("Refusing to generate a backup from invalid scan paths."));
        assert!(!script.contains("backup_item"));
    }

    #[test]
    fn special_files_are_not_added_to_the_copy_script() {
        let mut special = item(
            "/tmp/example-socket",
            "example-socket",
            Classification::Unknown,
            false,
            false,
        );
        special.is_file = false;

        let script = generate_backup_script(&[special]);
        assert!(script.contains("Unsupported or non-UTF-8 filesystem entries were skipped."));
        assert!(!script.contains("backup_item"));
    }

    #[test]
    fn non_utf8_paths_are_skipped_instead_of_using_lossy_source_paths() {
        let mut item = item(
            "/home/user/invalid-name",
            "invalid-name",
            Classification::Unknown,
            false,
            false,
        );
        item.path_is_utf8 = false;

        let script = generate_backup_script(&[item]);
        assert!(script.contains("non-UTF-8 filesystem entries were skipped"));
        assert!(!script.contains("backup_item"));
    }

    #[test]
    fn generated_script_with_unusual_file_names_parses_as_bash() -> std::io::Result<()> {
        let unusual_name = "file 'quoted' $(not-run) `literal`\nname";
        let scanned_item = item(
            &format!("/home/user/{unusual_name}"),
            unusual_name,
            Classification::Unknown,
            false,
            false,
        );
        let script = generate_backup_script(&[scanned_item]);

        let mut child = Command::new("bash")
            .arg("-n")
            .stdin(Stdio::piped())
            .spawn()?;
        let mut stdin = child.stdin.take().expect("stdin was piped");
        stdin.write_all(script.as_bytes())?;
        drop(stdin);
        assert!(child.wait()?.success());

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn generated_script_copies_selected_files_safely() -> std::io::Result<()> {
        use std::fs;
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temp = tempfile::tempdir()?;
        let source_root = temp.path().join("source");
        fs::create_dir_all(source_root.join("data/.cache"))?;
        fs::create_dir_all(source_root.join("config"))?;

        let marker = temp.path().join("command-was-run");
        let unusual_name = "$(touch command-was-run)";
        let source_file = source_root.join("config").join(unusual_name);
        fs::write(&source_file, b"sample config")?;
        fs::write(source_root.join("data/.cache/transient"), b"cache")?;

        let symlink_source = source_root.join("config/link");
        symlink(&source_file, &symlink_source)?;

        let items = vec![
            item(
                &source_root.join("data").to_string_lossy(),
                "data",
                Classification::Data,
                true,
                false,
            ),
            item(
                &source_root.join("data/.cache").to_string_lossy(),
                "data/.cache",
                Classification::Cache,
                true,
                false,
            ),
            item(
                &source_file.to_string_lossy(),
                &format!("config/{unusual_name}"),
                Classification::Config,
                false,
                false,
            ),
            item(
                &symlink_source.to_string_lossy(),
                "config/link",
                Classification::Unknown,
                false,
                true,
            ),
        ];
        let script = generate_backup_script(&items);
        let destination = temp.path().join("backup");

        let output = Command::new("bash")
            .arg("-c")
            .arg(script)
            .arg("backup.sh")
            .arg(&destination)
            .current_dir(temp.path())
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        let copied_file = destination.join("config").join("config").join(unusual_name);
        assert_eq!(fs::read(&copied_file)?, b"sample config");
        assert!(!destination.join("data/data/.cache").exists());
        assert!(!marker.exists(), "path text must not execute as shell code");
        assert!(
            fs::symlink_metadata(destination.join("unknown/config/link"))?
                .file_type()
                .is_symlink()
        );

        let destination_mode = fs::metadata(&destination)?.permissions().mode();
        let file_mode = fs::metadata(copied_file)?.permissions().mode();
        assert_eq!(destination_mode & 0o077, 0);
        assert_eq!(file_mode & 0o077, 0);

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn generated_script_refuses_to_recursively_copy_an_unexpected_directory() -> std::io::Result<()>
    {
        use std::fs;

        let temp = tempfile::tempdir()?;
        let source_directory = temp.path().join("source-dir");
        fs::create_dir_all(&source_directory)?;
        fs::write(source_directory.join("do-not-copy"), b"content")?;
        let item = item(
            &source_directory.to_string_lossy(),
            "source-dir",
            Classification::Config,
            false,
            false,
        );
        let script = generate_backup_script(&[item]);
        let destination = temp.path().join("backup");

        let output = Command::new("bash")
            .arg("-c")
            .arg(script)
            .arg("backup.sh")
            .arg(&destination)
            .current_dir(temp.path())
            .output()?;

        assert!(!output.status.success());
        assert!(!destination.join("config/source-dir/do-not-copy").exists());

        Ok(())
    }
}
