use std::fs;
use std::path::{Component, Path, PathBuf};

const NIX_STORE: &str = "/nix/store";

fn is_nix_store_path(path: &Path) -> bool {
    path.starts_with(NIX_STORE)
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
}

/// Resolves a path (handling symlinks recursively) and checks if it points to a path under `/nix/store/`.
/// Returns a tuple of (is_nix, canonicalized_path).
pub fn resolve_and_check_nix(path: &Path) -> (bool, Option<PathBuf>) {
    // First, check if the path itself is literally a symlink
    let is_symlink = path.is_symlink();

    match fs::canonicalize(path) {
        Ok(canonical) => {
            let is_nix = is_nix_store_path(&canonical);
            (is_nix, Some(canonical))
        }
        Err(_) => {
            // A dangling absolute symlink into the store can still be identified. Do not use a
            // substring match here: an unrelated target such as `/tmp/nix/store/file` is not
            // managed by Nix.
            if is_symlink && let Ok(target) = fs::read_link(path) {
                let is_nix = target.is_absolute() && is_nix_store_path(&target);
                return (is_nix, Some(target));
            }
            (false, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_resolve_and_check_nix_non_existent() {
        let path = Path::new("/nonexistent/path/somefile");
        let (is_nix, canonical) = resolve_and_check_nix(path);
        assert!(!is_nix);
        assert!(canonical.is_none());
    }

    #[test]
    fn test_resolve_and_check_nix_normal_file() -> std::io::Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test_file");
        File::create(&file_path)?;

        let (is_nix, canonical) = resolve_and_check_nix(&file_path);
        assert!(!is_nix);
        assert!(canonical.is_some());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_dangling_symlinks_only_match_absolute_nix_store_targets() -> std::io::Result<()> {
        use std::os::unix::fs::symlink;

        let dir = tempdir()?;
        let nix_link = dir.path().join("nix-link");
        symlink("/nix/store/not-present", &nix_link)?;
        let (is_nix, target) = resolve_and_check_nix(&nix_link);
        assert!(is_nix);
        assert_eq!(target, Some(PathBuf::from("/nix/store/not-present")));

        let fake_store_dir = dir.path().join("nested/nix/store");
        std::fs::create_dir_all(&fake_store_dir)?;
        let unrelated_link = dir.path().join("unrelated-link");
        symlink(fake_store_dir.join("not-present"), &unrelated_link)?;
        let (is_nix, _) = resolve_and_check_nix(&unrelated_link);
        assert!(!is_nix);

        let escaping_link = dir.path().join("escaping-link");
        symlink("/nix/store/../../tmp/not-present", &escaping_link)?;
        let (is_nix, _) = resolve_and_check_nix(&escaping_link);
        assert!(!is_nix);

        Ok(())
    }
}
