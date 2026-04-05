# OpenIdiom Architecture & Coding Standards

## Dependency Direction

```
cli/ → core/ → db/
cli/ → ai/  → db/
cli/ → output/
ai/  → core/ (for ParsedNote, Filter, etc.)
```

Nothing depends on `cli/`. Core logic is testable without CLI or database.
Database is an implementation detail behind core interfaces.

## Layer Responsibilities

### cli/ — Command layer
- Parses arguments (clap derive structs)
- Loads config, opens DB connection
- Calls into core/ or ai/ with concrete dependencies
- Formats output via output/
- Handles exit codes
- **No business logic here.** If you're writing an `if` that isn't about args or output, it belongs in core/.

### core/ — Domain logic
- Pure functions where possible: `parse_note(content: &str) -> Result<ParsedNote>`
- Stateful operations take dependencies as parameters, not globals:
  `index_vault(conn: &Connection, config: &VaultConfig) -> Result<IndexStats>`
- No direct filesystem access in parser — takes content as `&str`
  (filesystem walking is in the indexer, which is the boundary)
- No awareness of CLI flags or output formatting

### db/ — Persistence
- Schema definitions and migrations
- Prepared query functions: `fn get_notes_by_tag(conn: &Connection, tag: &str) -> Result<Vec<Note>>`
- Returns domain types, not raw rows
- All queries go through named functions — no inline SQL in core/ or cli/

### ai/ — AI integration
- Provider trait implementations
- Embedding and RAG orchestration
- Cost estimation
- Uses core/ types (ParsedNote, Filter) but doesn't depend on cli/

### output/ — Formatting
- Takes domain types, returns formatted strings
- Handles JSON, table, DOT, quiet modes
- No business logic, no DB access

### mcp/ — Protocol layer
- MCP server implementation
- Translates MCP tool calls → core/ function calls
- Same dependency direction as cli/ (it's just another frontend)

## Error Strategy

Two-layer approach:

```rust
// In core/, db/, ai/ — typed errors via thiserror
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("No vault found. Run `oi init` first.")]
    NotFound,
    #[error("Invalid config: {field} — {reason}")]
    InvalidConfig { field: String, reason: String },
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
}

// In cli/ — anyhow for ergonomic propagation and exit code mapping
fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            ExitCode::from(exit_code_for(&e))
        }
    }
}
```

Library code uses `thiserror` → precise, matchable errors.
CLI code uses `anyhow` → ergonomic `?` chaining, context attachment.
The boundary: cli/ functions return `anyhow::Result`, core/ functions return `Result<T, SpecificError>`.

## Config & Vault Pattern

```rust
// core/vault.rs
pub struct VaultConfig {
    pub name: String,
    pub daily_folder: String,
    pub daily_format: String,
    pub ignore: Vec<String>,
    pub ai: AiConfig,
}

pub struct Vault {
    pub root: PathBuf,
    pub config: VaultConfig,
    pub db_path: PathBuf,
}

impl Vault {
    /// Discover vault by walking up from current dir looking for .openidiom/
    pub fn discover(from: &Path) -> Result<Vault, VaultError> { ... }

    /// Open the SQLite connection
    pub fn open_db(&self) -> Result<Connection, VaultError> { ... }
}
```

Every command that needs a vault calls `Vault::discover(current_dir)` at the top.
Vault is passed by reference into core functions — no global state.

## Testing Strategy

### Unit tests (in-module `#[cfg(test)]`)
- Parser: feed content strings, assert ParsedNote fields
- Link resolver: feed title→path maps, assert resolution
- Config validation: feed TOML strings, assert errors
- Query filter: assert SQL generation from Filter enums

### Integration tests (tests/integration/)
- Use `tempfile::TempDir` as vault root
- Use `assert_cmd::Command` to run `oi` binary
- Fixture vault: `tests/fixtures/basic_vault/` with crafted .md files
- AI tests: mock HTTP server (e.g., `wiremock` crate) for provider tests

### What to test vs. what not to
- **Test:** every public function in core/, every CLI command's happy path + error path
- **Test:** edge cases in parser (empty files, frontmatter-only, nested wikilinks in code blocks)
- **Test:** cross-platform path handling (forward slash wikilinks on any OS)
- **Don't test:** clap argument parsing (clap's job), output formatting details (visual, fragile)

## Coding Standards

- Rust edition 2024
- `#![warn(clippy::all)]` in main.rs
- No `unwrap()` in library code — only in tests and main.rs error boundary
- All public types and trait methods get doc comments
- Private functions: comment only if non-obvious
- Prefer `&str` over `String` in function parameters where ownership isn't needed
- Prefer `impl Into<PathBuf>` or `AsRef<Path>` for path parameters
- No `clone()` without a reason — if you're cloning to satisfy the borrow checker,
  reconsider the data flow
- Modules: one file per module unless it's trivially small (< 30 lines)

## Naming Conventions

- Commands: `oi <verb>` (init, index, query, search, check, graph, daily, status)
- AI subcommands: `oi ai <verb>` (index, search, ask, connect, summarize, draft, metrics)
- MCP: `oi mcp serve`
- Structs: `PascalCase` (ParsedNote, VaultConfig, WikiLink)
- Functions: `snake_case` (parse_note, index_vault, resolve_links)
- Error enums: `<Module>Error` (VaultError, ParserError, AiError)
- DB query functions: `get_*`, `insert_*`, `update_*`, `delete_*`, `count_*`
