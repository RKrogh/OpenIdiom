# OpenIdiom

**Your Markdown files are a knowledge base. OpenIdiom makes them smart.**

A fast, local-first CLI that indexes your Markdown vault — wikilinks, tags, frontmatter — and layers AI-powered search, Q&A, and link discovery on top. Works alongside Obsidian, or standalone. No GUI needed.

```
$ oi index
Indexed 142 notes, 387 links, 95 tags (12 new, 3 updated)

$ oi search "error handling patterns"
 Title                 Path                        Relevance
 Error Handling        notes/error-handling.md     -2.34
 API Design Patterns   notes/api-design.md         -4.12

$ oi ai ask "What are our conventions for error handling?"
Based on your notes, you follow the Result pattern for error handling.
Domain errors are mapped to HTTP status codes in the API layer...

Sources: notes/error-handling.md, notes/api-design.md
```

---

## Why OpenIdiom?

| Problem | OpenIdiom's answer |
|---|---|
| Obsidian is closed-source and GUI-only | Open source Rust CLI, works in any terminal |
| Knowledge search means opening an app | `oi search "topic"` from anywhere |
| No AI integration in note tools | RAG-powered Q&A grounded in *your* notes |
| Tools lock you into their editor | Editor-agnostic — use VS Code, Neovim, whatever |
| Cloud-dependent sync | Local-first — your files, your machine, your SQLite |

---

## Quick Start

```bash
# Install (from source)
cargo install --path .

# Initialize a vault
cd ~/notes
oi init

# Index everything
oi index

# Start exploring
oi search "authentication"
oi query --tag backend
oi check --broken-links
oi graph --format dot | dot -Tsvg -o graph.svg
```

---

## Commands at a Glance

### Core

| Command | What it does |
|---|---|
| `oi init` | Initialize a vault in the current directory |
| `oi index` | Scan and index all Markdown files |
| `oi status` | Vault health: note count, index freshness, stale files |

### Query & Search

| Command | What it does |
|---|---|
| `oi search "keyword"` | Full-text keyword search (FTS5, instant, free) |
| `oi query --tag rust` | Find notes by tag |
| `oi query --link api-design` | Notes that link to a target |
| `oi query --backlink api-design` | Notes that a source links to |
| `oi query --orphan` | Notes with no links in or out |
| `oi query --front "status=draft"` | Filter by frontmatter fields |

### Vault Health

| Command | What it does |
|---|---|
| `oi check` | Run all health checks |
| `oi check --broken-links` | Find wikilinks pointing nowhere |
| `oi check --orphans` | Find disconnected notes |
| `oi check --dead-tags` | Tags used only once |

### Graph & Daily

| Command | What it does |
|---|---|
| `oi graph` | Export link graph as JSON |
| `oi graph --format dot` | Export as Graphviz DOT (pipe to `dot -Tsvg`) |
| `oi graph --root api-design --depth 2` | Ego graph: 2 hops from a note |
| `oi daily` | Create today's daily note, print the path |
| `oi daily --yesterday` | Yesterday's daily note |

### AI-Powered

| Command | What it does |
|---|---|
| `oi ai index` | Embed all notes for semantic search |
| `oi ai search "concept"` | Semantic search (meaning, not just keywords) |
| `oi ai ask "question"` | Ask a question answered from your notes (RAG) |
| `oi ai connect note-name` | Discover missing links between notes |
| `oi ai summarize --tag meeting` | Summarize notes matching a filter |
| `oi ai metrics` | View API usage and costs |

### Integration

| Command | What it does |
|---|---|
| `oi mcp serve` | Start MCP server for AI tool integration |
| `oi completions bash\|zsh\|fish` | Generate shell completions |

---

## Output Modes

Every query command supports multiple output formats:

```bash
oi query --tag rust              # Human-readable table (default)
oi query --tag rust --json       # JSON for piping to jq
oi query --tag rust --paths      # Just file paths, one per line
```

Compose with standard tools:

```bash
oi query --tag todo --paths | xargs nvim
oi graph --format json | jq '.nodes | length'
oi check --broken-links --json | jq '.[].path'
oi search "auth" --paths | head -3 | xargs cat
```

---

## AI Providers

OpenIdiom supports three AI backends. Mix and match — use one for completions, another for embeddings.

### Claude (default)

```toml
# .openidiom/config.toml
[ai]
provider = "claude"
model = "claude-sonnet-4-6"
embedding_provider = "ollama"           # Claude has no embedding API
embedding_model = "nomic-embed-text"
```

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

### OpenAI-Compatible

```toml
[ai]
provider = "openai"
model = "gpt-4"
embedding_provider = "openai"
embedding_model = "text-embedding-3-small"
# base_url = "https://api.openai.com/v1"  # or any compatible API
```

```bash
export OPENAI_API_KEY="sk-..."
```

### Ollama (fully local, free)

```toml
[ai]
provider = "ollama"
model = "llama3"
embedding_provider = "ollama"
embedding_model = "nomic-embed-text"
# ollama_url = "http://localhost:11434"
```

No API key needed. No data leaves your machine.

---

## MCP Server

OpenIdiom exposes your vault as an MCP server, so AI assistants like Claude Desktop can query your notes directly.

```bash
oi mcp serve    # Starts on stdio
```

Exposed tools: `vault_status`, `query_notes`, `search_notes`, `get_note`, `check_vault`

Add to your Claude Desktop config:

```json
{
  "mcpServers": {
    "openidiom": {
      "command": "oi",
      "args": ["mcp", "serve"],
      "cwd": "/path/to/your/vault"
    }
  }
}
```

---

## Use Cases

**Developer knowledge base**
```bash
# Index your team's architecture decision records
cd ~/docs/adr && oi init && oi index

# Find all notes about caching
oi search "caching strategy"

# What did we decide about auth?
oi ai ask "What authentication approach did we choose and why?"
```

**Writing & research**
```bash
# Daily journaling workflow
nvim $(oi daily)

# Find orphaned ideas that need connecting
oi check --orphans

# AI suggests links you might have missed
oi ai connect "thesis-chapter-3"
```

**Obsidian power-up**
```bash
# Run alongside Obsidian on the same vault
cd ~/obsidian-vault && oi init && oi index

# Get AI search without an Obsidian plugin
oi ai search "that thing about distributed consensus"

# Audit vault health in CI
oi check --broken-links --json || echo "Fix your links!"
```

---

## Configuration

`oi init` creates `.openidiom/config.toml`:

```toml
[vault]
name = "my-vault"
daily_folder = "daily"
daily_format = "%Y-%m-%d"
ignore = [".openidiom", ".git", "node_modules", ".obsidian"]

[ai]
provider = "claude"              # claude | openai | ollama
# model = "claude-sonnet-4-6"   # LLM for completions
embedding_provider = "openai"    # openai | ollama
embedding_model = "text-embedding-3-small"
# base_url = "..."              # For OpenAI-compatible APIs
# ollama_url = "http://localhost:11434"
chunk_size = 500                 # Tokens per embedding chunk
search_top_k = 10               # Semantic search results
context_top_k = 5                # RAG context chunks
batch_size = 50                  # Embeddings per API call
```

---

## Obsidian Compatibility

OpenIdiom reads standard Obsidian syntax:

- `[[wikilinks]]` and `[[target|alias]]`
- `#tags` and `#nested/tags`
- YAML frontmatter between `---` delimiters
- Case-insensitive link resolution (shortest path wins for ambiguity)

The `.openidiom/` folder is self-contained. Obsidian won't touch it, and OpenIdiom won't touch `.obsidian/`.

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Check found issues (broken links, orphans, etc.) |
| `2` | User error (bad arguments, invalid config) |
| `3` | System error (DB failure, API error) |

---

## Building from Source

```bash
git clone https://github.com/youruser/openidiom.git
cd openidiom
cargo build --release
# Binary at target/release/oi
```

Requires Rust 1.75+ (for native async traits). SQLite is bundled — no system dependencies.

Cross-platform: Linux, macOS, Windows.

---

## License

MIT
