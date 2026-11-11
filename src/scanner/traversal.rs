use crate::scanner::ignore::IgnoreList;
use crate::scanner::nix::resolve_and_check_nix;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
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
    pub path: String,
    pub target: Option<String>,
    pub classification: Classification,
    pub is_dir: bool,
    pub size: u64,
    pub description: Option<String>,
}

pub struct Scanner {
    root: PathBuf,
    ignore_list: IgnoreList,
}

impl Scanner {
    pub fn new(root: PathBuf, ignore_list: IgnoreList) -> Self {
        Scanner { root, ignore_list }
    }

    pub fn scan(&self) -> Vec<ScannedItem> {
        let mut items = Vec::new();
        let mut it = WalkDir::new(&self.root)
            .follow_links(false) // Handle symlinks ourselves to check targets and prune correctly
            .into_iter();

        loop {
            let entry = match it.next() {
                None => break,
                Some(Ok(entry)) => entry,
                Some(Err(_)) => continue, // Skip inaccessible files
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
            let (is_nix, target) = resolve_and_check_nix(path);

            let metadata = entry.metadata().ok();
            let is_dir =
                entry.file_type().is_dir() || (entry.file_type().is_symlink() && path.is_dir());
            let size = metadata.map(|m| m.len()).unwrap_or(0);

            if is_nix {
                items.push(ScannedItem {
                    path: path.to_string_lossy().to_string(),
                    target: target.map(|t| t.to_string_lossy().to_string()),
                    classification: Classification::Managed,
                    is_dir,
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
                target: target.map(|t| t.to_string_lossy().to_string()),
                classification,
                is_dir,
                size,
                description: Some(reason),
            });
        }

        items
    }

    fn classify_path(&self, path: &Path) -> (Classification, String) {
        let rel_path = path.strip_prefix(&self.root).unwrap_or(path);
        let path_str = rel_path.to_string_lossy().to_lowercase();
        let file_name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        // Check for Secrets
        if file_name.contains("id_rsa")
            || file_name.contains("id_ed25519")
            || file_name.contains("id_ecdsa")
            || path_str.starts_with(".ssh/")
            || path_str.contains("/.ssh/")
            || file_name == "credentials"
            || file_name == "auth.json"
            || file_name.contains("secret")
            || file_name.contains("token")
            || file_name == ".env"
        {
            return (
                Classification::Secrets,
                "Likely contains sensitive credentials or keys".to_string(),
            );
        }

        // Check for Cache
        if path_str.starts_with(".cache/")
            || path_str.contains("/.cache/")
            || path_str.starts_with("cache/")
            || path_str.contains("/cache/")
            || path_str.starts_with("tmp/")
            || path_str.contains("/tmp/")
            || path_str.starts_with("temp/")
            || path_str.contains("/temp/")
            || path_str.starts_with("node_modules/")
            || path_str.contains("/node_modules/")
            || path_str.contains("/.cargo/registry")
            || path_str.contains("/.cargo/git")
            || path_str.starts_with(".direnv/")
            || path_str.contains("/.direnv/")
            || path_str.starts_with(".npm/")
            || path_str.contains("/.npm/")
            || path_str.contains("/.pytest_cache/")
            || path_str.contains("/__pycache__/")
            || path_str.contains("/target/debug/")
            || path_str.contains("/target/release/")
            || file_name == "node_modules"
            || file_name == ".cache"
            || file_name == "target"
            || file_name == ".direnv"
            || file_name == "gcache"
        {
            return (
                Classification::Cache,
                "Transient build artifact, package or runtime cache".to_string(),
            );
        }

        // Check for Config
        if path_str.starts_with(".config/")
            || path_str.contains("/.config/")
            || file_name == ".config"
            || file_name.ends_with(".conf")
            || file_name.ends_with(".config")
            || file_name.ends_with(".json")
            || file_name.ends_with(".yaml")
            || file_name.ends_with(".yml")
            || file_name.ends_with(".toml")
            || file_name.ends_with(".ini")
            || file_name.ends_with("rc")
        // e.g. .bashrc, .zshrc
        {
            return (
                Classification::Config,
                "Application configuration file or directory".to_string(),
            );
        }

        // Check for Data
        if path_str.starts_with(".local/share/")
            || path_str.contains("/.local/share/")
            || file_name == ".local"
            || file_name == "share"
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
        let results = scanner.scan();

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

        Ok(())
    }
}
