# heap-snapshot

CLI tool for analyzing V8 `.heapsnapshot` files. Includes an interactive TUI viewer and several CLI subcommands for inspecting heap summaries, retainers, containment trees, diffs, and more.

This project was started from Chromium DevTools' `HeapSnapshot.ts` (`front_end/entrypoints/heap_snapshot_worker/HeapSnapshot.ts`), converted to Rust by AI. The original code is Copyright 2011 The Chromium Authors, licensed under a BSD-style license.

## Building

Requires Rust (edition 2024). Install via [rustup](https://rustup.rs/) if needed.

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

# Show retainers for an object
heap-snapshot retainers snapshot.heapsnapshot @3005313

# Show containment tree
heap-snapshot containment snapshot.heapsnapshot

# Compare two snapshots (CLI diff)
heap-snapshot diff main.heapsnapshot baseline.heapsnapshot

# Print stack roots
heap-snapshot stack snapshot.heapsnapshot

# Show unreachable objects
heap-snapshot unreachable snapshot.heapsnapshot

# Dump native context info
heap-snapshot contexts snapshot.heapsnapshot
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

## Running tests

```sh
cargo test
```

## Running benchmarks

```sh
cargo bench
```
