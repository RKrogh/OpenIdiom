use std::path::Path;

#[path = "../src/core/link_resolver.rs"]
mod link_resolver;

#[test]
fn test_resolve_exact_match() {
    let mut resolver = link_resolver::LinkResolver::new();
    resolver.register("api-design", Path::new("notes/api-design.md"));

    let result = resolver.resolve("api-design");
    assert!(result.resolved_path.is_some());
    assert_eq!(result.resolved_path.unwrap(), Path::new("notes/api-design.md"));
    assert!(!result.ambiguous);
}

#[test]
fn test_resolve_case_insensitive() {
    let mut resolver = link_resolver::LinkResolver::new();
    resolver.register("API-Design", Path::new("notes/API-Design.md"));

    let result = resolver.resolve("api-design");
    assert!(result.resolved_path.is_some());
    assert_eq!(result.resolved_path.unwrap(), Path::new("notes/API-Design.md"));
}

#[test]
fn test_resolve_not_found() {
    let resolver = link_resolver::LinkResolver::new();
    let result = resolver.resolve("nonexistent");
    assert!(result.resolved_path.is_none());
    assert!(!result.ambiguous);
    assert!(result.candidates.is_empty());
}

#[test]
fn test_resolve_ambiguous_shortest_path_wins() {
    let mut resolver = link_resolver::LinkResolver::new();
    resolver.register("note", Path::new("note.md"));
    resolver.register("note", Path::new("subfolder/note.md"));
    resolver.register("note", Path::new("deep/nested/note.md"));

    let result = resolver.resolve("note");
    assert!(result.ambiguous);
    assert_eq!(result.candidates.len(), 3);
    // Shortest path (fewest components) should win
    assert_eq!(result.resolved_path.unwrap(), Path::new("note.md"));
}

#[test]
fn test_resolve_explicit_path() {
    let mut resolver = link_resolver::LinkResolver::new();
    resolver.register("note", Path::new("note.md"));
    resolver.register("note", Path::new("subfolder/note.md"));

    // Explicit path should match exactly
    let result = resolver.resolve("subfolder/note");
    assert!(result.resolved_path.is_some());
    assert_eq!(result.resolved_path.unwrap(), Path::new("subfolder/note.md"));
    assert!(!result.ambiguous);
}

#[test]
fn test_resolve_multiple_registrations() {
    let mut resolver = link_resolver::LinkResolver::new();
    resolver.register("alpha", Path::new("alpha.md"));
    resolver.register("beta", Path::new("beta.md"));
    resolver.register("gamma", Path::new("nested/gamma.md"));

    assert!(resolver.resolve("alpha").resolved_path.is_some());
    assert!(resolver.resolve("beta").resolved_path.is_some());
    assert!(resolver.resolve("gamma").resolved_path.is_some());
    assert!(resolver.resolve("delta").resolved_path.is_none());
}
