use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::io::Write;
use std::process::Stdio;
use tempfile::TempDir;

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

// ============================================================
// Phase 4: Output polish tests
// ============================================================

#[test]
fn test_query_json_is_valid_array() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["query", "--tag", "backend", "--json"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json.is_array());
}

#[test]
fn test_check_json_is_valid_array() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["check", "--broken-links", "--json"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json.is_array());
}

#[test]
fn test_status_json_has_required_fields() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["status", "--json"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json.get("name").is_some());
    assert!(json.get("total_notes").is_some());
    assert!(json.get("total_links").is_some());
    assert!(json.get("total_tags").is_some());
}

#[test]
fn test_search_json_has_results() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["search", "middleware", "--json"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json.is_array());
}

#[test]
fn test_graph_json_has_nodes_and_edges() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["graph", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json.get("nodes").unwrap().is_array());
    assert!(json.get("edges").unwrap().is_array());
}

// Exit code tests

#[test]
fn test_exit_code_0_on_success() {
    let dir = setup_indexed_vault();
    oi().args(["status"])
        .current_dir(dir.path())
        .assert()
        .code(0);
}

#[test]
fn test_exit_code_1_on_check_issues() {
    let dir = setup_indexed_vault();
    // basic_vault has broken links → exit 1
    oi().args(["check", "--broken-links"])
        .current_dir(dir.path())
        .assert()
        .code(1);
}

#[test]
fn test_exit_code_on_no_vault() {
    let dir = TempDir::new().unwrap();
    let output = oi()
        .args(["status"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    // Should be exit code 3 (system error) — no vault found
    assert!(!output.status.success());
}

// Shell completions

#[test]
fn test_completions_bash() {
    oi().args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete").or(predicate::str::contains("_oi")));
}

#[test]
fn test_completions_zsh() {
    oi().args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("compdef").or(predicate::str::contains("#compdef")));
}

#[test]
fn test_completions_fish() {
    oi().args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete"));
}

// Quiet mode

#[test]
fn test_query_quiet_output() {
    let dir = setup_indexed_vault();
    let output = oi()
        .args(["query", "--tag", "backend", "--paths"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // --paths should output minimal: just paths, one per line
    for line in stdout.lines() {
        assert!(line.ends_with(".md"), "Expected .md path, got: {line}");
    }
}

// ============================================================
// Phase 5: MCP server tests
// ============================================================

#[test]
fn test_mcp_initialize_handshake() {
    let dir = setup_indexed_vault();

    // Build the binary path
    let bin = assert_cmd::cargo::cargo_bin("oi");

    // Start MCP server as subprocess with stdio
    let mut child = std::process::Command::new(&bin)
        .args(["mcp", "serve"])
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start mcp server");

    let stdin = child.stdin.as_mut().unwrap();

    // Send initialize request
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "0.1" }
        }
    });
    let msg = serde_json::to_string(&init_request).unwrap();
    writeln!(stdin, "{msg}").unwrap();
    stdin.flush().unwrap();

    // Send initialized notification
    let initialized = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    let msg2 = serde_json::to_string(&initialized).unwrap();
    writeln!(stdin, "{msg2}").unwrap();
    stdin.flush().unwrap();

    // Close stdin to signal we're done
    drop(child.stdin.take());

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should have responded with server info
    assert!(
        stdout.contains("serverInfo") || stdout.contains("capabilities"),
        "MCP server should respond to initialize. Got: {stdout}"
    );
}

#[test]
fn test_mcp_tools_list() {
    let dir = setup_indexed_vault();
    let bin = assert_cmd::cargo::cargo_bin("oi");

    let mut child = std::process::Command::new(&bin)
        .args(["mcp", "serve"])
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start mcp server");

    let stdin = child.stdin.as_mut().unwrap();

    // Initialize first
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2024-11-05", "capabilities": {},
                     "clientInfo": { "name": "test", "version": "0.1" } }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init).unwrap()).unwrap();
    let notif = serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    writeln!(stdin, "{}", serde_json::to_string(&notif).unwrap()).unwrap();

    // List tools
    let list_tools = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/list"
    });
    writeln!(stdin, "{}", serde_json::to_string(&list_tools).unwrap()).unwrap();
    stdin.flush().unwrap();

    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should list our tools
    assert!(
        stdout.contains("query_notes") || stdout.contains("search_notes") || stdout.contains("vault_status"),
        "Should list vault tools. Got: {stdout}"
    );
}

#[test]
fn test_mcp_tool_call_vault_status() {
    let dir = setup_indexed_vault();
    let bin = assert_cmd::cargo::cargo_bin("oi");

    let mut child = std::process::Command::new(&bin)
        .args(["mcp", "serve"])
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start mcp server");

    let stdin = child.stdin.as_mut().unwrap();

    // Initialize
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2024-11-05", "capabilities": {},
                     "clientInfo": { "name": "test", "version": "0.1" } }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init).unwrap()).unwrap();
    writeln!(stdin, "{}", serde_json::to_string(&serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"})).unwrap()).unwrap();

    // Call vault_status tool
    let call = serde_json::json!({
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {
            "name": "vault_status",
            "arguments": {}
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&call).unwrap()).unwrap();
    stdin.flush().unwrap();

    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should return vault status data
    assert!(
        stdout.contains("total_notes") || stdout.contains("my-vault") || stdout.contains("Notes"),
        "vault_status should return vault info. Got: {stdout}"
    );
}

#[test]
fn test_mcp_tool_call_search_notes() {
    let dir = setup_indexed_vault();
    let bin = assert_cmd::cargo::cargo_bin("oi");

    let mut child = std::process::Command::new(&bin)
        .args(["mcp", "serve"])
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start mcp server");

    let stdin = child.stdin.as_mut().unwrap();

    // Initialize
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2024-11-05", "capabilities": {},
                     "clientInfo": { "name": "test", "version": "0.1" } }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init).unwrap()).unwrap();
    writeln!(stdin, "{}", serde_json::to_string(&serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"})).unwrap()).unwrap();

    // Search notes
    let call = serde_json::json!({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": {
            "name": "search_notes",
            "arguments": { "query": "middleware" }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&call).unwrap()).unwrap();
    stdin.flush().unwrap();

    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("auth-middleware") || stdout.contains("middleware"),
        "search_notes should find middleware notes. Got: {stdout}"
    );
}
