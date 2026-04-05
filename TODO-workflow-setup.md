# OpenIdiom Workflow Setup Plan

## Goal
All Claude Code sessions across all projects share a growing knowledge base.
Cross-machine sync via Syncthing. Every session contributes, every session benefits.

## Architecture

```
~/Projects/                          <-- the vault (oi init here)
├── .openidiom/                      <-- index DB + config
├── notes/                           <-- cross-cutting knowledge (ADRs, patterns, incidents)
│   ├── architecture/
│   ├── patterns/
│   └── decisions/
├── ProjectA/                        <-- each project as normal
├── ProjectB/
└── ...
```

## Setup Tasks

### 1. Initialize the vault at ~/Projects
- [ ] `cd ~/Projects && oi init`
- [ ] Tune `.openidiom/config.toml` ignore list for the multi-repo setup:
      add target, bin, obj, dist, .next, packages, *.dll, *.exe etc.
- [ ] Set AI provider to claude + ollama for embeddings (local-first)
- [ ] First full index: `oi index --stats`
- [ ] Verify: `oi status`, `oi search "test"`, `oi check`

### 2. Create the notes/ structure
- [ ] `mkdir -p ~/Projects/notes/{architecture,patterns,decisions,incidents}`
- [ ] Seed with 2-3 starter notes (an architecture decision, a pattern you reuse)
- [ ] Tag conventions: #decision, #pattern, #incident, #project/<name>
- [ ] Verify they index: `oi index && oi query --tag decision`

### 3. Global CLAUDE.md instructions
- [ ] Add oi awareness to ~/.claude/CLAUDE.md
- [ ] Instructions for sessions to: search before deciding, document after deciding
- [ ] Test: start a new Claude Code session, verify it knows about oi

### 4. Re-indexing automation
- [ ] Option A: cron job (simple) — `*/15 * * * * cd ~/Projects && oi index`
- [ ] Option B: Claude Code hook — runs `oi index` after commits
- [ ] Pick one and set it up
- [ ] Verify: make a change, wait for re-index, search for it

### 5. Syncthing setup
- [ ] Install Syncthing on all machines
  - Windows: `winget install Syncthing.Syncthing` or https://syncthing.net/downloads/
  - WSL: `sudo apt install syncthing`
  - Other machines: same
- [ ] Configure ~/Projects as a shared folder between devices
- [ ] Ignore patterns in Syncthing (.stignore):
  ```
  node_modules
  target
  bin/Debug
  bin/Release
  obj
  .next
  dist
  *.db-journal
  ```
- [ ] Decision: sync .openidiom/index.db or re-index per machine?
  Recommendation: add `index.db` to .stignore and re-index locally.
  Config.toml syncs, DB rebuilds. Avoids SQLite lock conflicts.
- [ ] Test: create a note on machine A, verify it appears on machine B,
  run `oi index` on B, search for it

### 6. Verification
- [ ] Start a Claude Code session in ProjectA
- [ ] Ask it to check for related architecture decisions
- [ ] Verify it uses oi to search across projects
- [ ] Start a session in ProjectB, verify cross-project context works
- [ ] Document anything that needs adjusting

## Notes
- Embedding (oi ai index) is optional for start — keyword search works without it
- Embeddings cost money if using OpenAI. Ollama embeddings are free but need Ollama running.
- Start with keyword search, add AI features once the vault has enough content to justify it
