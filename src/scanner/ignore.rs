use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct IgnoreList {
    patterns: Vec<String>,
}

impl IgnoreList {
    /// Loads patterns from a .nix-migrate-ignore file.
    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(IgnoreList::default());
            }
            Err(error) => return Err(error),
        };

        let mut patterns = Vec::new();
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            patterns.push(trimmed.to_string());
        }
        Ok(IgnoreList { patterns })
    }

    /// Checks if a given path is ignored relative to the root scan directory.
    pub fn is_ignored(&self, path: &Path, root: &Path) -> bool {
        let rel_path = path.strip_prefix(root).unwrap_or(path);
        let rel_str = rel_path.to_string_lossy();

        for pattern in &self.patterns {
            if let Some(clean_pattern) = pattern.strip_prefix('/') {
                if rel_str == clean_pattern || rel_str.starts_with(&format!("{}/", clean_pattern)) {
                    return true;
                }
            } else {
                for component in rel_path.components() {
                    if component.as_os_str().to_str() == Some(pattern) {
                        return true;
                    }
                }
                if rel_str == *pattern
                    || rel_str.contains(&format!("/{}/", pattern))
                    || rel_str.starts_with(&format!("{}/", pattern))
                    || rel_str.ends_with(&format!("/{}", pattern))
                {
                    return true;
                }
            }
        }
        false
    }

    /// Get a list of the parsed patterns.
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_ignore_list() -> std::io::Result<()> {
        let dir = tempdir()?;
        let ignore_file_path = dir.path().join(".nix-migrate-ignore");

        let mut file = File::create(&ignore_file_path)?;
        writeln!(file, "# This is a comment")?;
        writeln!(file, "")?;
        writeln!(file, "node_modules")?;
        writeln!(file, "/absolute_only")?;
        writeln!(file, ".cache")?;

        let list = IgnoreList::load_from_file(&ignore_file_path)?;
        assert_eq!(list.patterns().len(), 3);

        let root = dir.path();

        // Testing relative only pattern: node_modules
        assert!(list.is_ignored(&root.join("node_modules"), root));
        assert!(list.is_ignored(&root.join("subfolder").join("node_modules"), root));
        assert!(list.is_ignored(
            &root.join("subfolder").join("node_modules").join("nested"),
            root
        ));

        // Testing absolute/root prefix pattern: /absolute_only
        assert!(list.is_ignored(&root.join("absolute_only"), root));
        assert!(list.is_ignored(&root.join("absolute_only").join("nested"), root));
        assert!(!list.is_ignored(&root.join("subfolder").join("absolute_only"), root));

        // Non-ignored paths
        assert!(!list.is_ignored(&root.join("src"), root));
        assert!(!list.is_ignored(&root.join("Cargo.toml"), root));

        Ok(())
    }

    #[test]
    fn missing_ignore_file_means_no_patterns() -> std::io::Result<()> {
        let dir = tempdir()?;
        let missing_file = dir.path().join("does-not-exist");

        let list = IgnoreList::load_from_file(&missing_file)?;
        assert!(list.patterns().is_empty());

        Ok(())
    }
}
