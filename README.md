# heap-snapshot

CLI tool for analyzing V8 `.heapsnapshot` files. Includes an interactive TUI viewer and several CLI subcommands for inspecting heap summaries, retainers, containment trees, diffs, and more.

This project was started from Chromium DevTools' `HeapSnapshot.ts` (`front_end/entrypoints/heap_snapshot_worker/HeapSnapshot.ts`), converted to Rust by AI. The original code is Copyright 2011 The Chromium Authors, licensed under a BSD-style license.

## Installation

Install from npm:

```sh
# Run without installing
npx @heap-snapshot/tool

# Install globally
npm install -g @heap-snapshot/tool
```

Or build from source (requires Rust edition 2024, install via [rustup](https://rustup.rs/)):

```sh
cargo build --release
```

The binary will be at `target/release/heap-snapshot`.

## Usage

```sh
# Interactive TUI viewer
heap-snapshot tui snapshot.heapsnapshot

# Compare snapshots in TUI
heap-snapshot tui main.heapsnapshot baseline.heapsnapshot

# Print summary table
heap-snapshot summary snapshot.heapsnapshot

# Summary of unreachable objects only
heap-snapshot summary --unreachable snapshot.heapsnapshot

# Print heap statistics
heap-snapshot statistics snapshot.heapsnapshot

# Show retainers for an object
heap-snapshot retainers snapshot.heapsnapshot @3005313

# Show containment tree
heap-snapshot containment snapshot.heapsnapshot

# Compare two snapshots (CLI diff)
heap-snapshot diff main.heapsnapshot baseline.heapsnapshot

# Print stack roots
heap-snapshot stack snapshot.heapsnapshot

# Dump native context info
heap-snapshot contexts snapshot.heapsnapshot

# Print allocation timeline (for snapshots with allocation tracking)
heap-snapshot timeline snapshot.heaptimeline
```

### Snapshot options

These flags can be passed to any subcommand:

```sh
# Treat weak edges as reachable when computing distances.
# Objects referenced only via weak edges get distance+1 of the
# retainer instead of being marked unreachable (U).
heap-snapshot tui --weak-is-reachable snapshot.heapsnapshot
```

Run `heap-snapshot --help` or `heap-snapshot <subcommand> --help` for full option details.

## Web UI

The web UI is a Solid.js app in `web/` that uses a WASM build of the Rust core.

### Development

```sh
cd web
npm install
npm run dev        # builds WASM + starts vite dev server
```

### Available scripts

```sh
npm run build:wasm   # build the WASM module
npm run dev          # build WASM + start dev server
npm run build        # build WASM + production build
npm run typecheck    # run TypeScript type checking
npm run fmt          # format code with prettier
npm run test:e2e     # run Playwright end-to-end tests
```

## MCP Server

The binary also implements an MCP server (`heap-snapshot mcp`). Add it from the command line:

```sh
# Claude Code
claude mcp add heap-snapshot -- npx -y @heap-snapshot/tool mcp

# Codex
codex mcp add heap-snapshot -- npx -y @heap-snapshot/tool mcp

# Gemini CLI
gemini mcp add heap-snapshot npx -- -y @heap-snapshot/tool mcp
```

Or add this to the respective settings file (`.claude/settings.json`, `codex.json`, or `~/.gemini/settings.json`):

```json
{
  "mcpServers": {
    "heap-snapshot": {
      "command": "npx",
      "args": ["-y", "@heap-snapshot/tool", "mcp"]
    }
  }
}
```

## Running tests

Run all tests and checks with:

```sh
./test.py
```

This runs Rust formatting and tests, builds WASM, checks TypeScript types, verifies prettier formatting, and runs Playwright e2e tests. Requires [uv](https://docs.astral.sh/uv/).

Individual test suites can also be run separately:

```sh
cargo test                     # Rust unit + e2e tests
cd web && npm run test:e2e     # Playwright e2e tests (requires WASM built)
```

## Running benchmarks

```sh
cargo bench
```
