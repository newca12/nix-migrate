use crate::scanner::ignore::IgnoreList;
use crate::scanner::nix::resolve_and_check_nix;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum Classification {
    Managed,
    Config,
    Data,
    Cache,
    Secrets,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScannedItem {
    /// Lossy string representation of the absolute path of the scanned entry.
    pub path: String,
    /// Whether the path and its relative form are valid UTF-8 strings.
    #[serde(default = "default_true")]
    pub path_is_utf8: bool,
    /// Path relative to the selected scan root.
    #[serde(default)]
    pub relative_path: String,
    /// Symlink target, canonicalized when it can be resolved.
    pub target: Option<String>,
    /// Heuristic category assigned to this entry.
    pub classification: Classification,
    /// Whether this entry resolves to a directory.
    pub is_dir: bool,
    /// Whether this entry is a regular file (not a symlink or special file).
    #[serde(default = "default_true")]
    pub is_file: bool,
    /// Whether this entry is a symlink.
    #[serde(default)]
    pub is_symlink: bool,
    /// Entry metadata size in bytes (not recursive), or zero when unavailable.
    pub size: u64,
    /// Human-readable reason for the classification.
    pub description: Option<String>,
}

fn default_true() -> bool {
    true
}

pub struct Scanner {
    root: PathBuf,
    ignore_list: IgnoreList,
}

impl Scanner {
    pub fn new(root: PathBuf, ignore_list: IgnoreList) -> Self {
        Scanner { root, ignore_list }
    }

    pub fn scan(&self) -> Result<Vec<ScannedItem>, walkdir::Error> {
        let mut items = Vec::new();
        let mut it = WalkDir::new(&self.root)
            .follow_links(false) // Handle symlinks ourselves to check targets and prune correctly
            .into_iter();

        loop {
            let entry = match it.next().transpose()? {
                None => break,
                Some(entry) => entry,
            };

            let path = entry.path();

            // 1. Check if the path is ignored via .nix-migrate-ignore
            if self.ignore_list.is_ignored(path, &self.root) {
                if entry.file_type().is_dir() {
                    it.skip_current_dir();
                }
                continue;
            }

            // 2. Resolve symlink and check if it is managed by Nix
            let is_symlink = entry.file_type().is_symlink();
            let is_file = entry.file_type().is_file();
            let (is_nix, resolved_target) = resolve_and_check_nix(path);
            let target = is_symlink.then_some(resolved_target).flatten();

            let metadata = entry.metadata().ok();
            let is_dir =
                entry.file_type().is_dir() || (entry.file_type().is_symlink() && path.is_dir());
            let size = metadata.map(|m| m.len()).unwrap_or(0);
            let relative_path = path
                .strip_prefix(&self.root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            let path_is_utf8 = path.to_str().is_some()
                && path
                    .strip_prefix(&self.root)
                    .unwrap_or(path)
                    .to_str()
                    .is_some();

            if is_nix {
                items.push(ScannedItem {
                    path: path.to_string_lossy().to_string(),
                    path_is_utf8,
                    relative_path,
                    target: target.map(|t| t.to_string_lossy().to_string()),
                    classification: Classification::Managed,
                    is_dir,
                    is_file,
                    is_symlink,
                    size,
                    description: Some("Managed by Nix (/nix/store)".to_string()),
                });

                // Early pruning rule: If directory itself is symlinked to /nix/store, do NOT walk its children.
                if is_dir {
                    it.skip_current_dir();
                }
                continue;
            }

            // Skip the root scan directory itself from the output items to keep report clean,
            // but we must traverse it.
            if path == self.root {
                continue;
            }

            // 3. Classify the non-managed path
            let (classification, reason) = self.classify_path(path);

            items.push(ScannedItem {
                path: path.to_string_lossy().to_string(),
                path_is_utf8,
                relative_path,
                target: target.map(|t| t.to_string_lossy().to_string()),
                classification,
                is_dir,
                is_file,
                is_symlink,
                size,
                description: Some(reason),
            });
        }

        Ok(items)
    }

    fn classify_path(&self, path: &Path) -> (Classification, String) {
        let rel_path = path.strip_prefix(&self.root).unwrap_or(path);
        let file_name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let components: Vec<String> = rel_path
            .components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(name.to_string_lossy().to_lowercase()),
                _ => None,
            })
            .collect();
        let has_component = |name: &str| components.iter().any(|component| component == name);
        let has_component_pair = |first: &str, second: &str| {
            components
                .windows(2)
                .any(|pair| pair[0] == first && pair[1] == second)
        };

        // Check for Secrets
        let env_file = file_name == ".env" || file_name.starts_with(".env.");
        let ssh_private_key = (file_name.starts_with("id_rsa")
            || file_name.starts_with("id_ed25519")
            || file_name.starts_with("id_ecdsa")
            || file_name.starts_with("id_dsa"))
            && !file_name.ends_with(".pub");
        if has_component(".ssh")
            || has_component(".gnupg")
            || ssh_private_key
            || file_name.contains("credential")
            || file_name == "auth.json"
            || file_name.contains("secret")
            || file_name.contains("token")
            || file_name == ".netrc"
            || file_name.ends_with(".pem")
            || file_name.ends_with(".key")
            || file_name.ends_with(".p12")
            || file_name.ends_with(".pfx")
            || (has_component_pair(".docker", "config.json"))
            || env_file
        {
            return (
                Classification::Secrets,
                "Likely contains sensitive credentials or keys".to_string(),
            );
        }

        // Check for Cache
        if [
            ".cache",
            "cache",
            "tmp",
            "temp",
            "node_modules",
            ".direnv",
            ".npm",
            ".pytest_cache",
            "__pycache__",
            "target",
            "gcache",
        ]
        .iter()
        .any(|name| has_component(name))
            || has_component_pair(".cargo", "registry")
            || has_component_pair(".cargo", "git")
        {
            return (
                Classification::Cache,
                "Transient build artifact, package or runtime cache".to_string(),
            );
        }

        // Check for Config
        let rc_file = file_name.ends_with(".rc")
            || (file_name.starts_with('.') && file_name.ends_with("rc"))
            || matches!(
                file_name.as_str(),
                ".profile" | ".bash_profile" | ".zprofile" | ".zshenv"
            );
        if has_component(".config")
            || file_name.ends_with(".conf")
            || file_name.ends_with(".config")
            || file_name.ends_with(".json")
            || file_name.ends_with(".yaml")
            || file_name.ends_with(".yml")
            || file_name.ends_with(".toml")
            || file_name.ends_with(".ini")
            || rc_file
        {
            return (
                Classification::Config,
                "Application configuration file or directory".to_string(),
            );
        }

        // Check for Data
        if has_component_pair(".local", "share")
            || has_component(".local")
            || file_name.ends_with(".db")
            || file_name.ends_with(".sqlite")
            || file_name.ends_with(".sqlite3")
            || file_name.ends_with(".png")
            || file_name.ends_with(".jpg")
            || file_name.ends_with(".jpeg")
            || file_name.ends_with(".pdf")
            || file_name.ends_with(".mp3")
            || file_name.ends_with(".mp4")
            || file_name.ends_with(".txt")
            || file_name.ends_with(".md")
        {
            return (
                Classification::Data,
                "User data, documents, media, or local application database".to_string(),
            );
        }

        // Default to Unknown
        (
            Classification::Unknown,
            "Unclassified file or directory".to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_scanner_traversal() -> std::io::Result<()> {
        let dir = tempdir()?;
        let root = dir.path();

        // Create some files
        let cache_dir = root.join(".cache");
        std::fs::create_dir(&cache_dir)?;
        File::create(cache_dir.join("some_cache_file"))?;

        let config_dir = root.join(".config");
        std::fs::create_dir(&config_dir)?;
        let my_conf = config_dir.join("app.toml");
        File::create(&my_conf)?;

        let secrets_file = root.join("id_rsa");
        File::create(&secrets_file)?;

        let ignore_list = IgnoreList::default();
        let scanner = Scanner::new(root.to_path_buf(), ignore_list);
        let results = scanner.scan().expect("temporary tree should be readable");

        // Verify we found and classified the files correctly
        let app_toml_item = results
            .iter()
            .find(|i| i.path.ends_with("app.toml"))
            .unwrap();
        assert_eq!(app_toml_item.classification, Classification::Config);

        let secrets_item = results.iter().find(|i| i.path.ends_with("id_rsa")).unwrap();
        assert_eq!(secrets_item.classification, Classification::Secrets);

        let cache_item = results
            .iter()
            .find(|i| i.path.ends_with("some_cache_file"))
            .unwrap();
        assert_eq!(cache_item.classification, Classification::Cache);
        assert_eq!(cache_item.relative_path, ".cache/some_cache_file");

        Ok(())
    }

    #[test]
    fn test_classifies_nested_caches_and_secret_paths() -> std::io::Result<()> {
        let dir = tempdir()?;
        let root = dir.path();

        for path in [
            ".cargo/registry/index",
            "target/debug/deps/build-file",
            ".ssh/known_hosts",
        ] {
            let file_path = root.join(path);
            std::fs::create_dir_all(file_path.parent().expect("file has a parent"))?;
            File::create(file_path)?;
        }
        File::create(root.join(".env.local"))?;

        let scanner = Scanner::new(root.to_path_buf(), IgnoreList::default());
        let results = scanner.scan().expect("temporary tree should be readable");

        let classification_for = |suffix: &str| {
            results
                .iter()
                .find(|item| item.path.ends_with(suffix))
                .map(|item| item.classification)
                .expect("expected path to be scanned")
        };

        assert_eq!(
            classification_for(".cargo/registry/index"),
            Classification::Cache
        );
        assert_eq!(
            classification_for("target/debug/deps/build-file"),
            Classification::Cache
        );
        assert_eq!(
            classification_for(".ssh/known_hosts"),
            Classification::Secrets
        );
        assert_eq!(classification_for(".env.local"), Classification::Secrets);

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_marks_non_utf8_paths_as_unrepresentable() -> std::io::Result<()> {
        use std::os::unix::ffi::OsStringExt;

        let dir = tempdir()?;
        let file_name = std::ffi::OsString::from_vec(b"invalid-\xff-name".to_vec());
        File::create(dir.path().join(file_name))?;

        let scanner = Scanner::new(dir.path().to_path_buf(), IgnoreList::default());
        let results = scanner.scan().expect("temporary tree should be readable");
        let item = results
            .iter()
            .find(|item| !item.path_is_utf8)
            .expect("non-UTF-8 path should still be reported");

        assert!(item.path.contains('\u{fffd}'));
        assert!(item.relative_path.contains('\u{fffd}'));

        Ok(())
    }
}
