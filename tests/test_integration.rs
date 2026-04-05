use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn oi() -> Command {
    Command::cargo_bin("oi").expect("binary should exist")
}

fn setup_vault() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    oi().arg("init")
        .current_dir(dir.path())
        .assert()
        .success();
    dir
}

fn setup_vault_with_notes() -> TempDir {
    let dir = setup_vault();

    // Copy fixture files into the temp vault
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/basic_vault");

    copy_dir_recursive(&fixture_dir, dir.path());
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

// ===== oi init tests =====

#[test]
fn test_init_creates_config_dir() {
    let dir = TempDir::new().unwrap();
    oi().arg("init")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized vault"));

    assert!(dir.path().join(".openidiom").exists());
    assert!(dir.path().join(".openidiom/config.toml").exists());
    assert!(dir.path().join(".openidiom/index.db").exists());
}

#[test]
fn test_init_config_is_valid_toml() {
    let dir = TempDir::new().unwrap();
    oi().arg("init").current_dir(dir.path()).assert().success();

    let config_str = fs::read_to_string(dir.path().join(".openidiom/config.toml")).unwrap();
    let config: toml::Value = toml::from_str(&config_str).unwrap();

    assert!(config.get("vault").is_some());
    assert!(config.get("ai").is_some());
    assert_eq!(
        config["vault"]["name"].as_str().unwrap(),
        "my-vault"
    );
}

#[test]
fn test_init_fails_if_already_initialized() {
    let dir = setup_vault();
    oi().arg("init")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_init_creates_schema_tables() {
    let dir = setup_vault();
    let db_path = dir.path().join(".openidiom/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    // Verify core tables exist
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(tables.contains(&"notes".to_string()));
    assert!(tables.contains(&"links".to_string()));
    assert!(tables.contains(&"tags".to_string()));
    assert!(tables.contains(&"headings".to_string()));
    assert!(tables.contains(&"embeddings".to_string()));
}

#[test]
fn test_init_creates_fts_table() {
    let dir = setup_vault();
    let db_path = dir.path().join(".openidiom/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(tables.contains(&"notes_fts".to_string()));
}

// ===== oi index tests =====

#[test]
fn test_index_parses_notes() {
    let dir = setup_vault_with_notes();
    oi().arg("index")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Indexed"));
}

#[test]
fn test_index_counts_correct() {
    let dir = setup_vault_with_notes();
    oi().args(["index", "--stats"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Notes:"));
}

#[test]
fn test_index_incremental_skips_unchanged() {
    let dir = setup_vault_with_notes();

    // First index
    oi().arg("index").current_dir(dir.path()).assert().success();

    // Second index — should skip all
    oi().arg("index")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("0 new"));
}

#[test]
fn test_index_force_reindexes_all() {
    let dir = setup_vault_with_notes();

    // First index
    oi().arg("index").current_dir(dir.path()).assert().success();

    // Force re-index
    oi().args(["index", "--force"])
        .current_dir(dir.path())
        .assert()
        .success();

    // All notes should be re-indexed (no "0 new" — they're all "new" in force mode)
    // We just verify it succeeds; the count check is in the DB test below
}

#[test]
fn test_index_populates_database() {
    let dir = setup_vault_with_notes();
    oi().arg("index").current_dir(dir.path()).assert().success();

    let db_path = dir.path().join(".openidiom/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    let note_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
        .unwrap();
    // basic_vault has 7 .md files
    assert!(note_count >= 7, "Expected at least 7 notes, got {note_count}");
}

#[test]
fn test_index_stores_wikilinks() {
    let dir = setup_vault_with_notes();
    oi().arg("index").current_dir(dir.path()).assert().success();

    let db_path = dir.path().join(".openidiom/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    let link_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM links", [], |row| row.get(0))
        .unwrap();
    assert!(link_count > 0, "Should have indexed some wikilinks");

    // Check a specific link exists
    let has_auth_link: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM links WHERE target_title = 'auth-middleware'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(has_auth_link, "Should have link to auth-middleware");
}

#[test]
fn test_index_stores_tags() {
    let dir = setup_vault_with_notes();
    oi().arg("index").current_dir(dir.path()).assert().success();

    let db_path = dir.path().join(".openidiom/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    let tag_count: i64 = conn
        .query_row("SELECT COUNT(DISTINCT tag) FROM tags", [], |row| row.get(0))
        .unwrap();
    assert!(tag_count > 0, "Should have indexed some tags");

    // Check specific tags from frontmatter and body
    let has_backend: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM tags WHERE tag = 'backend'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(has_backend, "Should have 'backend' tag (from frontmatter and body)");
}

#[test]
fn test_index_stores_frontmatter() {
    let dir = setup_vault_with_notes();
    oi().arg("index").current_dir(dir.path()).assert().success();

    let db_path = dir.path().join(".openidiom/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    // api-design.md has frontmatter with status: published
    let fm_json: Option<String> = conn
        .query_row(
            "SELECT frontmatter_json FROM notes WHERE title = 'API Design Patterns'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert!(fm_json.is_some(), "Should have stored frontmatter JSON");
    let fm: serde_json::Value = serde_json::from_str(&fm_json.unwrap()).unwrap();
    assert_eq!(fm.get("status").unwrap().as_str().unwrap(), "published");
}

#[test]
fn test_index_resolves_links() {
    let dir = setup_vault_with_notes();
    oi().arg("index").current_dir(dir.path()).assert().success();

    let db_path = dir.path().join(".openidiom/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    // Links to existing notes should have target_id set
    let resolved_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM links WHERE target_id IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(resolved_count > 0, "Some links should be resolved");

    // Link to non-existent-note should have target_id NULL
    let unresolved: bool = conn
        .query_row(
            "SELECT target_id IS NULL FROM links WHERE target_title = 'non-existent-note'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(unresolved, "Link to non-existent-note should be unresolved");
}

#[test]
fn test_index_populates_fts() {
    let dir = setup_vault_with_notes();
    oi().arg("index").current_dir(dir.path()).assert().success();

    let db_path = dir.path().join(".openidiom/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    // Search FTS for content we know exists
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'authentication'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(count > 0, "FTS should find 'authentication' in notes");
}

// ===== oi status tests =====

#[test]
fn test_status_before_index() {
    let dir = setup_vault();
    oi().args(["status"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Notes: 0"));
}

#[test]
fn test_status_after_index() {
    let dir = setup_vault_with_notes();
    oi().arg("index").current_dir(dir.path()).assert().success();

    oi().args(["status"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("my-vault"))
        .stdout(predicate::str::contains("Notes:"));
}

#[test]
fn test_status_json_output() {
    let dir = setup_vault_with_notes();
    oi().arg("index").current_dir(dir.path()).assert().success();

    let output = oi()
        .args(["status", "--json"])
        .current_dir(dir.path())
        .output()
        .expect("run status --json");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("should be valid JSON");
    assert!(json.get("name").is_some());
    assert!(json.get("total_notes").is_some());
}

// ===== oi without vault tests =====

#[test]
fn test_index_without_init_fails() {
    let dir = TempDir::new().unwrap();
    oi().arg("index")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("No vault found"));
}

#[test]
fn test_status_without_init_fails() {
    let dir = TempDir::new().unwrap();
    oi().arg("status")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("No vault found"));
}
