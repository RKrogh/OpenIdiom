# OpenIdiom — Project Configuration

## What is this?

A Rust CLI tool (`oi`) that treats a folder of Markdown files as a knowledge base.
Indexes wikilinks, tags, frontmatter; provides query/search/graph tools; AI layer for
semantic search and RAG. Obsidian-compatible, editor-agnostic, local-first.

## Tech Stack

- **Language:** Rust (latest stable, 1.75+ for native async traits)
- **CLI:** clap (derive)
- **Database:** SQLite via rusqlite (bundled), including FTS5
- **Parsing:** gray_matter (frontmatter), regex (wikilinks/tags), pulldown-cmark (headings only)
- **AI:** reqwest + tokio for API calls, streaming SSE support
- **Error handling:** thiserror (library), anyhow (CLI)
- **YAML:** serde_yml (NOT serde_yaml — deprecated)

## Conventions

- Binary name: `oi`
- Config dir: `.openidiom/` in vault root
- Config file: `.openidiom/config.toml`
- Database: `.openidiom/index.db`
- File-scoped modules, no nested module files unless necessary
- All public commands support `--json` output
- Exit codes: 0 = success, 1 = check issues, 2 = user error, 3 = system/API error
- Config validation: fail fast with clear error messages on invalid config

## Architecture Notes

- Parser pipeline: gray_matter → regex (wikilinks, tags) → pulldown-cmark (headings)
- Link resolution: Obsidian-compatible (case-insensitive, shortest-path-wins for ambiguity)
- FTS5 keyword search is separate from AI semantic search
- Embedding storage: byte offsets + preview, NOT full chunk text
- AI provider traits use native async (no async_trait crate)
- Streaming is the default for LLM responses

## Plan

See `idea` file in this directory for the full implementation plan.
