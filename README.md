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

# AI features (requires Ollama)
oi ai setup                     # check config, get install instructions
ollama pull nomic-embed-text    # embedding model
oi ai index                     # embed your notes
oi ai search "some concept"    # semantic search
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

OpenIdiom supports three AI backends. Mix and match: use one for completions, another for embeddings.

**Run `oi ai setup` at any time to check your configuration and get guided install instructions.**

### Ollama (default, fully local, free)

Out of the box, OpenIdiom uses [Ollama](https://ollama.com) for everything. No API keys, no accounts, no data leaves your machine.

```bash
# 1. Install Ollama
#    Linux:   curl -fsSL https://ollama.com/install.sh | sh
#    macOS:   brew install ollama
#    Windows: winget install Ollama.Ollama

# 2. Pull the models
ollama pull nomic-embed-text    # embeddings (required)
ollama pull llama3              # LLM for ask/summarize/connect (pick any model you like)

# 3. That's it. Default config works.
oi ai index
oi ai search "some concept"
```

### Claude

Better quality for RAG, summarization, and link discovery. Requires an API key. Note that Claude does not offer an embedding API, so embeddings still use Ollama or OpenAI.

```toml
[ai]
provider = "claude"
# model = "claude-sonnet-4-6"
embedding_provider = "ollama"
embedding_model = "nomic-embed-text"
```

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

### OpenAI-Compatible

Works with OpenAI, Azure OpenAI, or any compatible API.

```toml
[ai]
provider = "openai"
model = "gpt-4"
embedding_provider = "openai"
embedding_model = "text-embedding-3-small"
# base_url = "https://api.openai.com/v1"  # or any compatible endpoint
```

```bash
export OPENAI_API_KEY="sk-..."
```

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

## AI Agent Integration

OpenIdiom works as a knowledge layer for AI coding agents, not just for humans. If you use Claude Code, Cursor, Copilot, or similar tools, your agent can query your notes for context, prior decisions, and cross-project knowledge.

The key insight: your AI agent doesn't need the LLM provider. It *is* the LLM. It only needs `oi` for search and retrieval. Embeddings run locally via Ollama, so there's nothing to configure.

### How it works

```bash
# Agent uses keyword search (no AI needed)
oi search "authentication patterns"

# Agent uses semantic search (needs embeddings only, runs locally)
oi ai search "how we handle retries in the payment service"

# Agent uses structured queries
oi query --tag architecture
oi query --link api-design
```

The agent reads the results, synthesizes them with its own capabilities, and brings your accumulated knowledge into the current session.

### Example: Claude Code global instructions

Add something like this to your global `CLAUDE.md` (or equivalent for your AI tool) to make `oi` available across all projects:

```markdown
## Knowledge Base

You have access to `oi`, a CLI that indexes Markdown notes as a knowledge base.
Use it proactively when searching for context, prior decisions, or cross-project knowledge.

### Quick reference
oi search "query"          # Keyword search across the vault
oi query --tag sometag     # Query by tags, links, frontmatter
oi ai search "question"   # Semantic search (local embeddings)
oi status                  # Vault overview
oi check                   # Vault health checks

The vault is initialized at ~/projects/ and indexes all project folders.
Before searching, ensure the index is current: oi index
```

### Write knowledge, not just read it

Instruct your AI agent to write notes when a session produces knowledge worth preserving. Non-obvious decisions, architecture patterns, debugging insights, cross-project connections. The bar: "would a future session benefit from finding this?"

```markdown
When a session produces reusable knowledge, write a note to ~/projects/notes/ai/:

---
title: Short descriptive title
tags: [topic, project-name]
date: 2026-01-15
---

Body text with [[wikilinks]] to related notes.
```

This creates a feedback loop: sessions produce knowledge that future sessions can find.

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
provider = "ollama"              # ollama | claude | openai
# model = "llama3"              # LLM for completions
embedding_provider = "ollama"    # ollama | openai
embedding_model = "nomic-embed-text"
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
