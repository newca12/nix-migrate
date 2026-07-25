use std::path::{Path, PathBuf};
use std::fs;

/// Resolves a path (handling symlinks recursively) and checks if it points to a path under `/nix/store/`.
/// Returns a tuple of (is_nix, canonicalized_path).
pub fn resolve_and_check_nix(path: &Path) -> (bool, Option<PathBuf>) {
    // First, check if the path itself is literally a symlink
    let is_symlink = path.is_symlink();

    match fs::canonicalize(path) {
        Ok(canonical) => {
            let is_nix = canonical.starts_with("/nix/store/");
            (is_nix, Some(canonical))
        }
        Err(_) => {
            // If canonicalize failed, maybe it is a broken symlink or some target in Nix store that we cannot access.
            // Let's try to read the link directly.
            if is_symlink {
                let target = fs::read_link(path);
                if let Ok(target) = target {
                    let is_nix = target.starts_with("/nix/store/") || target.to_string_lossy().contains("/nix/store/");
                    return (is_nix, Some(target));
                }
            }
            (false, None)
        }
    }
}

/// Checks if a directory itself is a symlink pointing to the Nix store.
/// This is crucial for early recursion pruning during scanning.
pub fn is_nix_managed_directory(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    
    // If it's a symlink, check its destination
    if path.is_symlink() {
        let (is_nix, _) = resolve_and_check_nix(path);
        return is_nix;
    }

    false
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
}
