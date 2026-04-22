use std::path::Path;

use globset::{Glob, GlobSet, GlobSetBuilder};

const OIIGNORE_FILE: &str = ".oiignore";

/// Compiled ignore rules from config.toml ignore list + .oiignore file.
pub struct IgnoreRules {
    /// Simple name matches from config.toml (e.g. ".git", "node_modules")
    config_names: Vec<String>,
    /// Glob patterns from .oiignore
    globs: GlobSet,
}

impl IgnoreRules {
    /// Build ignore rules from config ignore list + .oiignore at vault root.
    pub fn load(vault_root: &Path, config_ignore: &[String]) -> Result<Self, IgnoreError> {
        let config_names = config_ignore.to_vec();

        let oiignore_path = vault_root.join(OIIGNORE_FILE);
        let mut builder = GlobSetBuilder::new();

        if oiignore_path.is_file() {
            let content = std::fs::read_to_string(&oiignore_path)
                .map_err(IgnoreError::Io)?;
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let glob = Glob::new(line).map_err(|e| IgnoreError::Pattern {
                    pattern: line.to_string(),
                    reason: e.to_string(),
                })?;
                builder.add(glob);
            }
        }

        let globs = builder.build().map_err(|e| IgnoreError::Pattern {
            pattern: String::new(),
            reason: e.to_string(),
        })?;

        Ok(Self { config_names, globs })
    }

    /// Check if a path (relative to vault root) should be ignored.
    pub fn is_ignored(&self, rel_path: &Path) -> bool {
        // Check config name matches (any path component matches)
        for component in rel_path.components() {
            let name = component.as_os_str().to_string_lossy();
            for pattern in &self.config_names {
                if *name == **pattern {
                    return true;
                }
            }
        }

        // Check .oiignore glob patterns against the relative path
        let path_str = rel_path.to_string_lossy();
        // Match against both the full relative path and just the filename
        if self.globs.is_match(rel_path) {
            return true;
        }
        // Also try matching with forward slashes (cross-platform)
        if rel_path.to_string_lossy().contains('\\') {
            let normalized = path_str.replace('\\', "/");
            if self.globs.is_match(normalized.as_str()) {
                return true;
            }
        }

        false
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IgnoreError {
    #[error("IO error reading .oiignore: {0}")]
    Io(std::io::Error),
    #[error("Invalid ignore pattern '{pattern}': {reason}")]
    Pattern { pattern: String, reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules_from(config_ignore: &[&str], oiignore_content: Option<&str>) -> IgnoreRules {
        let dir = tempfile::tempdir().unwrap();
        if let Some(content) = oiignore_content {
            std::fs::write(dir.path().join(".oiignore"), content).unwrap();
        }
        let config: Vec<String> = config_ignore.iter().map(|s| s.to_string()).collect();
        IgnoreRules::load(dir.path(), &config).unwrap()
    }

    #[test]
    fn test_config_name_match() {
        let rules = rules_from(&[".git", "node_modules"], None);
        assert!(rules.is_ignored(Path::new(".git")));
        assert!(rules.is_ignored(Path::new("node_modules")));
        assert!(rules.is_ignored(Path::new("some/deep/node_modules/file.md")));
        assert!(!rules.is_ignored(Path::new("notes/readme.md")));
    }

    #[test]
    fn test_oiignore_glob_extension() {
        let rules = rules_from(&[], Some("*.dll\n*.exe\n"));
        assert!(rules.is_ignored(Path::new("bin/app.dll")));
        assert!(rules.is_ignored(Path::new("app.exe")));
        assert!(!rules.is_ignored(Path::new("notes/readme.md")));
    }

    #[test]
    fn test_oiignore_directory_glob() {
        let rules = rules_from(&[], Some("target/**\n**/bin/Debug/**\n"));
        assert!(rules.is_ignored(Path::new("target/debug/build")));
        assert!(rules.is_ignored(Path::new("project/bin/Debug/net8.0/app.dll")));
        assert!(!rules.is_ignored(Path::new("notes/readme.md")));
    }

    #[test]
    fn test_oiignore_comments_and_blanks() {
        let rules = rules_from(&[], Some("# This is a comment\n\n*.tmp\n  # indented comment\n"));
        assert!(rules.is_ignored(Path::new("file.tmp")));
        assert!(!rules.is_ignored(Path::new("file.md")));
    }

    #[test]
    fn test_combined_config_and_oiignore() {
        let rules = rules_from(&[".git"], Some("*.dll\n"));
        assert!(rules.is_ignored(Path::new(".git")));
        assert!(rules.is_ignored(Path::new("lib/thing.dll")));
        assert!(!rules.is_ignored(Path::new("notes/readme.md")));
    }

    #[test]
    fn test_no_oiignore_file() {
        let rules = rules_from(&[".git"], None);
        assert!(rules.is_ignored(Path::new(".git")));
        assert!(!rules.is_ignored(Path::new("readme.md")));
    }

    #[test]
    fn test_invalid_glob_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".oiignore"), "[invalid").unwrap();
        let result = IgnoreRules::load(dir.path(), &[]);
        assert!(result.is_err());
    }
}
