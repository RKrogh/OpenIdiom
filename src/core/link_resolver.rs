use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Resolves wikilink targets to note paths using Obsidian-compatible rules:
/// 1. Case-insensitive filename match
/// 2. Shortest path wins for ambiguity
/// 3. Explicit path prefixes match exactly
pub struct LinkResolver {
    /// Lowercase title → list of (path, title) pairs
    title_map: HashMap<String, Vec<(PathBuf, String)>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ResolvedLink {
    pub target_title: String,
    pub resolved_path: Option<PathBuf>,
    pub ambiguous: bool,
    pub candidates: Vec<PathBuf>,
}

impl LinkResolver {
    pub fn new() -> Self {
        Self {
            title_map: HashMap::new(),
        }
    }

    /// Register a note by its title and path
    pub fn register(&mut self, title: &str, path: &Path) {
        let key = title.to_lowercase();
        self.title_map
            .entry(key)
            .or_default()
            .push((path.to_path_buf(), title.to_string()));
    }

    /// Resolve a wikilink target to a note path
    pub fn resolve(&self, target: &str) -> ResolvedLink {
        let key = target.to_lowercase();

        // Check for explicit path (contains /)
        if target.contains('/') {
            // Look for exact path suffix match
            for entries in self.title_map.values() {
                for (path, _) in entries {
                    let path_str = path.to_string_lossy().replace('\\', "/");
                    if path_str.ends_with(&format!("{target}.md"))
                        || path_str.ends_with(target)
                    {
                        return ResolvedLink {
                            target_title: target.to_string(),
                            resolved_path: Some(path.clone()),
                            ambiguous: false,
                            candidates: vec![path.clone()],
                        };
                    }
                }
            }
        }

        match self.title_map.get(&key) {
            None => ResolvedLink {
                target_title: target.to_string(),
                resolved_path: None,
                ambiguous: false,
                candidates: vec![],
            },
            Some(entries) if entries.len() == 1 => ResolvedLink {
                target_title: target.to_string(),
                resolved_path: Some(entries[0].0.clone()),
                ambiguous: false,
                candidates: vec![entries[0].0.clone()],
            },
            Some(entries) => {
                // Multiple matches — shortest path wins
                let mut sorted: Vec<_> = entries.iter().collect();
                sorted.sort_by_key(|(p, _)| p.components().count());
                let candidates: Vec<PathBuf> = sorted.iter().map(|(p, _)| p.clone()).collect();

                ResolvedLink {
                    target_title: target.to_string(),
                    resolved_path: Some(sorted[0].0.clone()),
                    ambiguous: true,
                    candidates,
                }
            }
        }
    }
}
