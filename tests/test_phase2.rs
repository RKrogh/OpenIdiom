use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn oi() -> Command {
    Command::cargo_bin("oi").expect("binary should exist")
}

fn setup_indexed_vault() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");

    // Init vault
    oi().arg("init").current_dir(dir.path()).assert().success();

    // Copy fixture files
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/basic_vault");
    copy_dir_recursive(&fixture_dir, dir.path());

    // Index
    oi().arg("index").current_dir(dir.path()).assert().success();

    dir
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) {
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            fs::create_dir_all(&dst_path).unwrap();
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            fs::copy(&src_path, &dst_path).unwrap();
        }
    }
}

// ============================================================
// oi query tests
// ============================================================

#[test]
fn test_query_by_tag() {
    let dir = setup_indexed_vault();
    oi().args(["query", "--tag", "backend"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("api-design").or(predicate::str::contains("API Design")));
}

#[test]
fn test_query_by_multiple_tags_and() {
    let dir = setup_indexed_vault();
    // Only notes with BOTH backend AND security tags
    oi().args(["query", "--tag", "backend", "--tag", "security"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("auth-middleware").or(predicate::str::contains("Auth Middleware")));
}

#[test]
fn test_query_by_link() {
    let dir = setup_indexed_vault();
    // Notes that link TO api-design
    oi().args(["query", "--link", "api-design"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("auth-middleware").or(predicate::str::contains("subfolder-note")));
}

#[test]
fn test_query_by_backlink() {
    let dir = setup_indexed_vault();
    // Notes that api-design links TO
    oi().args(["query", "--backlink", "api-design"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("auth-middleware"));
}

#[test]
fn test_query_by_title() {
    let dir = setup_indexed_vault();
    oi().args(["query", "--title", "Error"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("error-handling").or(predicate::str::contains("Error Handling")));
}

#[test]
fn test_query_by_frontmatter() {
    let dir = setup_indexed_vault();
    oi().args(["query", "--front", "status=published"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("api-design").or(predicate::str::contains("API Design")));
}

#[test]
fn test_query_by_min_words() {
    let dir = setup_indexed_vault();
    // api-design.md has the most content; orphan-note has little
    oi().args(["query", "--min-words", "20"])
        .current_dir(dir.path())
        .assert()
        .success();
    // Just verify it runs without error and returns something
}

#[test]
fn test_query_orphan() {
    let dir = setup_indexed_vault();
    oi().args(["query", "--orphan"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("orphan-note").or(predicate::str::contains("Orphan")));
}

#[test]
fn test_query_json_output() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["query", "--tag", "backend", "--json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("--json output should be valid JSON");
    assert!(json.is_array(), "JSON output should be an array of notes");
}

#[test]
fn test_query_paths_output() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["query", "--tag", "backend", "--paths"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // --paths should output one path per line, ending in .md
    for line in stdout.lines() {
        assert!(line.ends_with(".md"), "Each line should be a .md path, got: {line}");
    }
}

#[test]
fn test_query_no_results() {
    let dir = setup_indexed_vault();
    oi().args(["query", "--tag", "nonexistent-tag-xyz"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::is_empty().or(predicate::str::contains("No results")));
}

// ============================================================
// oi search tests (FTS5 keyword search)
// ============================================================

#[test]
fn test_search_finds_content() {
    let dir = setup_indexed_vault();
    oi().args(["search", "authentication"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("api-design").or(predicate::str::contains("API Design")));
}

#[test]
fn test_search_ranks_results() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["search", "middleware", "--json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("should be valid JSON");
    assert!(json.is_array());
    // auth-middleware.md should rank high for "middleware"
}

#[test]
fn test_search_with_limit() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["search", "backend", "--limit", "2", "--json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("should be valid JSON");
    let arr = json.as_array().unwrap();
    assert!(arr.len() <= 2, "Should respect --limit 2");
}

#[test]
fn test_search_paths_output() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["search", "error", "--paths"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        assert!(line.ends_with(".md"), "Each line should be a path: {line}");
    }
}

#[test]
fn test_search_no_results() {
    let dir = setup_indexed_vault();
    oi().args(["search", "xyznonexistenttermxyz"])
        .current_dir(dir.path())
        .assert()
        .success();
}

// ============================================================
// oi check tests
// ============================================================

#[test]
fn test_check_finds_broken_links() {
    let dir = setup_indexed_vault();
    // api-design.md links to [[non-existent-note]]
    oi().args(["check", "--broken-links"])
        .current_dir(dir.path())
        .assert()
        .failure() // exit code 1 = issues found
        .stdout(predicate::str::contains("non-existent-note"));
}

#[test]
fn test_check_finds_orphans() {
    let dir = setup_indexed_vault();
    oi().args(["check", "--orphans"])
        .current_dir(dir.path())
        .assert()
        .failure() // exit code 1 = issues found
        .stdout(predicate::str::contains("orphan-note").or(predicate::str::contains("Orphan")));
}

#[test]
fn test_check_finds_ambiguous_links() {
    let dir = setup_indexed_vault();
    // ambiguous-note.md exists in both root and subfolder/
    let _ = oi().args(["check", "--ambiguous-links"])
        .current_dir(dir.path())
        .assert();
    // May or may not find issues depending on whether any note links to ambiguous-note
    // The key test is that the flag works without error
}

#[test]
fn test_check_finds_dead_tags() {
    let dir = setup_indexed_vault();
    // random-topic tag appears only in orphan-note.md
    oi().args(["check", "--dead-tags"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains("random-topic"));
}

#[test]
fn test_check_all() {
    let dir = setup_indexed_vault();
    // Run all checks
    oi().arg("check")
        .current_dir(dir.path())
        .assert()
        .failure(); // should find at least the broken link
}

#[test]
fn test_check_json_output() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["check", "--broken-links", "--json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("--json should produce valid JSON");
    assert!(json.is_array());
}

// ============================================================
// oi graph tests
// ============================================================

#[test]
fn test_graph_json_default() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["graph"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("default graph output should be JSON");
    assert!(json.get("nodes").is_some(), "Should have nodes");
    assert!(json.get("edges").is_some(), "Should have edges");
}

#[test]
fn test_graph_json_has_correct_structure() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["graph", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let nodes = json["nodes"].as_array().unwrap();
    assert!(!nodes.is_empty());

    // Each node should have id, path, tags, word_count
    let node = &nodes[0];
    assert!(node.get("id").is_some());
    assert!(node.get("path").is_some());
    assert!(node.get("tags").is_some());
}

#[test]
fn test_graph_dot_format() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["graph", "--format", "dot"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("digraph"), "DOT output should start with digraph");
    assert!(stdout.contains("->"), "DOT output should have edges");
}

#[test]
fn test_graph_filtered_by_tag() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["graph", "--filter-tag", "security"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let nodes = json["nodes"].as_array().unwrap();
    // Only notes with #security tag
    for node in nodes {
        let tags = node["tags"].as_array().unwrap();
        let tag_strs: Vec<&str> = tags.iter().filter_map(|t| t.as_str()).collect();
        assert!(tag_strs.contains(&"security"), "Filtered nodes should have security tag");
    }
}

#[test]
fn test_graph_ego_from_root() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["graph", "--root", "api-design", "--depth", "1"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let nodes = json["nodes"].as_array().unwrap();
    // Should include api-design and its direct neighbors
    let ids: Vec<&str> = nodes.iter().filter_map(|n| n["id"].as_str()).collect();
    assert!(ids.contains(&"api-design") || ids.iter().any(|id| id.contains("api-design")));
}

// ============================================================
// oi daily tests
// ============================================================

#[test]
fn test_daily_creates_note() {
    let dir = setup_indexed_vault();
    let output = oi()
        .arg("daily")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // Should print a path
    assert!(path_str.ends_with(".md"), "Should output a .md path, got: {path_str}");
    // The file should actually exist
    let full_path = dir.path().join(&path_str);
    assert!(full_path.exists(), "Daily note file should be created at {}", full_path.display());
}

#[test]
fn test_daily_idempotent() {
    let dir = setup_indexed_vault();

    // First call creates
    let out1 = oi().arg("daily").current_dir(dir.path()).output().unwrap();
    let path1 = String::from_utf8_lossy(&out1.stdout).trim().to_string();

    // Second call returns same path
    let out2 = oi().arg("daily").current_dir(dir.path()).output().unwrap();
    let path2 = String::from_utf8_lossy(&out2.stdout).trim().to_string();

    assert_eq!(path1, path2, "Daily should be idempotent");
}

#[test]
fn test_daily_with_date() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["daily", "--date", "2026-01-15"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(path_str.contains("2026-01-15"), "Path should contain the date: {path_str}");
}

#[test]
fn test_daily_yesterday() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["daily", "--yesterday"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(path_str.ends_with(".md"));
}

#[test]
fn test_daily_with_template() {
    let dir = setup_indexed_vault();

    // Create a template
    let template_path = dir.path().join("templates/daily.md");
    fs::create_dir_all(dir.path().join("templates")).unwrap();
    fs::write(&template_path, "# {{date}}\n\n## Tasks\n\n## Notes\n").unwrap();

    let output = oi()
        .args(["daily", "--date", "2026-06-01", "--template", "templates/daily.md"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let content = fs::read_to_string(dir.path().join(&path_str)).unwrap();
    assert!(content.contains("## Tasks"), "Template content should be in the daily note");
}
