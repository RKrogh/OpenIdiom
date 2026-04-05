use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;
use wiremock::{Mock, MockServer, ResponseTemplate};
use wiremock::matchers::{method, path};

fn oi() -> Command {
    Command::cargo_bin("oi").expect("binary should exist")
}

fn setup_indexed_vault() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    oi().arg("init").current_dir(dir.path()).assert().success();

    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/basic_vault");
    copy_dir_recursive(&fixture_dir, dir.path());

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

/// Helper: patch vault config to point at a mock server
fn patch_config_for_mock(dir: &TempDir, base_url: &str) {
    let config_path = dir.path().join(".openidiom/config.toml");
    let config = format!(
        r#"[vault]
name = "test-vault"
daily_folder = "daily"
daily_format = "%Y-%m-%d"
ignore = [".openidiom", ".git", "node_modules", ".obsidian"]

[ai]
provider = "openai"
model = "gpt-4"
embedding_provider = "openai"
embedding_model = "text-embedding-3-small"
base_url = "{base_url}/v1"
chunk_size = 500
search_top_k = 10
context_top_k = 5
batch_size = 50
"#
    );
    fs::write(config_path, config).unwrap();
}

/// Mock embedding response matching OpenAI's format
fn mock_embedding_response(dimension: usize, count: usize) -> serde_json::Value {
    let embeddings: Vec<serde_json::Value> = (0..count)
        .map(|i| {
            let vec: Vec<f64> = (0..dimension).map(|d| ((i + d) as f64) * 0.01).collect();
            serde_json::json!({
                "object": "embedding",
                "index": i,
                "embedding": vec
            })
        })
        .collect();

    serde_json::json!({
        "object": "list",
        "data": embeddings,
        "model": "text-embedding-3-small",
        "usage": { "prompt_tokens": 100, "total_tokens": 100 }
    })
}

/// Mock chat completion response matching OpenAI's format
fn mock_chat_response(content: &str) -> serde_json::Value {
    serde_json::json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": content
            },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 50, "completion_tokens": 30, "total_tokens": 80 }
    })
}

// ============================================================
// Provider factory tests
// ============================================================

#[test]
fn test_provider_factory_creates_openai() {
    // With OPENAI_API_KEY set and provider=openai, factory should succeed
    let dir = setup_indexed_vault();
    patch_config_for_mock(&dir, "http://localhost:1234");

    // oi ai search with no embeddings should give a clear message, not crash
    oi().args(["ai", "search", "test"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn test_missing_api_key_error() {
    let dir = setup_indexed_vault();
    patch_config_for_mock(&dir, "http://localhost:1234");

    // Remove any API key from env — should fail with clear message
    oi().args(["ai", "ask", "test question"])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("API key").or(predicate::str::contains("api key").or(predicate::str::contains("OPENAI_API_KEY"))));
}

#[test]
fn test_provider_factory_ollama_no_key_needed() {
    let dir = setup_indexed_vault();
    let config_path = dir.path().join(".openidiom/config.toml");
    let config = r#"[vault]
name = "test-vault"
daily_folder = "daily"
daily_format = "%Y-%m-%d"
ignore = [".openidiom", ".git"]

[ai]
provider = "ollama"
model = "llama3"
embedding_provider = "ollama"
embedding_model = "nomic-embed-text"
ollama_url = "http://localhost:11434"
"#;
    fs::write(config_path, config).unwrap();

    // Ollama provider should not require API key (will fail connecting, but not on key)
    let output = oi()
        .args(["ai", "search", "test"])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should NOT complain about missing API key
    assert!(
        !stderr.contains("API key") && !stderr.contains("api key"),
        "Ollama should not require API key, got: {stderr}"
    );
}

// ============================================================
// Embedding indexer tests
// ============================================================

#[tokio::test]
async fn test_ai_index_embeds_notes() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_embedding_response(1536, 50)))
        .expect(1..)
        .mount(&mock_server)
        .await;

    let dir = setup_indexed_vault();
    patch_config_for_mock(&dir, &mock_server.uri());

    oi().args(["ai", "index", "--yes"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Embedded").or(predicate::str::contains("embedded")));

    // Verify embeddings in DB
    let db_path = dir.path().join(".openidiom/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))
        .unwrap();
    assert!(count > 0, "Should have stored embeddings, got {count}");
}

#[tokio::test]
async fn test_ai_index_incremental() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_embedding_response(1536, 50)))
        .expect(1..)
        .mount(&mock_server)
        .await;

    let dir = setup_indexed_vault();
    patch_config_for_mock(&dir, &mock_server.uri());

    // First embed
    oi().args(["ai", "index", "--yes"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success();

    // Second embed should skip (no changes)
    oi().args(["ai", "index", "--yes"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("skip").or(predicate::str::contains("0 new")).or(predicate::str::contains("up to date")));
}

#[tokio::test]
async fn test_ai_index_dry_run() {
    let dir = setup_indexed_vault();
    patch_config_for_mock(&dir, "http://localhost:9999");

    // --dry-run should NOT call the API (no server needed)
    oi().args(["ai", "index", "--dry-run"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("token").or(predicate::str::contains("cost")).or(predicate::str::contains("estimate")));
}

// ============================================================
// AI search (semantic vector search) tests
// ============================================================

#[tokio::test]
async fn test_ai_search_returns_results() {
    let mock_server = MockServer::start().await;

    // Mock for indexing
    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_embedding_response(1536, 50)))
        .expect(1..)
        .mount(&mock_server)
        .await;

    let dir = setup_indexed_vault();
    patch_config_for_mock(&dir, &mock_server.uri());

    // Index first
    oi().args(["ai", "index", "--yes"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success();

    // Now search
    oi().args(["ai", "search", "authentication"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success();
}

#[tokio::test]
async fn test_ai_search_no_embeddings_message() {
    let dir = setup_indexed_vault();
    patch_config_for_mock(&dir, "http://localhost:9999");

    // Search without having indexed embeddings
    oi().args(["ai", "search", "test"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("No embeddings")
                .or(predicate::str::contains("no embeddings"))
                .or(predicate::str::contains("Run `oi ai index`"))
        );
}

// ============================================================
// AI ask (RAG) tests
// ============================================================

#[tokio::test]
async fn test_ai_ask_returns_answer() {
    let mock_server = MockServer::start().await;

    // Mock embedding endpoint (for indexing and query embedding)
    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_embedding_response(1536, 50)))
        .expect(1..)
        .mount(&mock_server)
        .await;

    // Mock chat completion endpoint
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_chat_response(
                "Based on your notes, error handling follows the Result pattern. Sources: error-handling.md"
            ))
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let dir = setup_indexed_vault();
    patch_config_for_mock(&dir, &mock_server.uri());

    // Index embeddings first
    oi().args(["ai", "index", "--yes"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success();

    // Ask a question
    oi().args(["ai", "ask", "How do we handle errors?", "--no-stream"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Result pattern").or(predicate::str::contains("error")));
}

// ============================================================
// AI connect tests
// ============================================================

#[tokio::test]
async fn test_ai_connect_suggests_links() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_embedding_response(1536, 50)))
        .expect(1..)
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_chat_response(
                "api-design.md could link to: error-handling.md (both discuss HTTP patterns)"
            ))
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let dir = setup_indexed_vault();
    patch_config_for_mock(&dir, &mock_server.uri());

    oi().args(["ai", "index", "--yes"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success();

    oi().args(["ai", "connect", "api-design", "--no-stream"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success();
}

// ============================================================
// AI summarize tests
// ============================================================

#[tokio::test]
async fn test_ai_summarize_by_tag() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_chat_response(
                "Summary: The backend notes cover API design, authentication middleware, and error handling."
            ))
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let dir = setup_indexed_vault();
    patch_config_for_mock(&dir, &mock_server.uri());

    oi().args(["ai", "summarize", "--tag", "backend", "--no-stream"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Summary").or(predicate::str::contains("backend")));
}

// ============================================================
// AI metrics tests
// ============================================================

#[tokio::test]
async fn test_ai_metrics_shows_usage() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_embedding_response(1536, 50)))
        .expect(1..)
        .mount(&mock_server)
        .await;

    let dir = setup_indexed_vault();
    patch_config_for_mock(&dir, &mock_server.uri());

    // Generate some usage
    oi().args(["ai", "index", "--yes"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success();

    // Check metrics
    oi().args(["ai", "metrics"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("token").or(predicate::str::contains("Token")));
}

// ============================================================
// Cost estimation tests
// ============================================================

#[test]
fn test_ai_index_shows_cost_estimate() {
    let dir = setup_indexed_vault();
    patch_config_for_mock(&dir, "http://localhost:9999");

    // --dry-run should show token/cost info
    oi().args(["ai", "index", "--dry-run"])
        .env("OPENAI_API_KEY", "test-key")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("token").or(predicate::str::contains("cost")).or(predicate::str::contains("notes")));
}

// ============================================================
// Config validation for AI section
// ============================================================

#[test]
fn test_config_extended_ai_fields_accepted() {
    let dir = setup_indexed_vault();
    patch_config_for_mock(&dir, "http://localhost:9999");

    // Config with all AI fields should be accepted
    oi().args(["status"])
        .current_dir(dir.path())
        .assert()
        .success();
}
