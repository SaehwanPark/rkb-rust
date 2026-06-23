# ARCHITECTURE

## Overview

`rkb-rust` is organized as one library crate and one `rkb` binary. The binary
owns process concerns; library modules own typed domain transformations, retrieval
formatting, evaluation reports, and thin side-effect adapters for preservation,
extraction, parsing, variable metadata, provenance QA, lexical/hybrid retrieval,
MCP serving/setup, downstream integration helpers, and progress summaries.

Last Reviewed: 2026-06-22
Status: Verified
## Boundaries

```text
CLI parsing -> typed command -> pure domain pipeline -> I/O adapter -> artifact
```

- `src/main.rs` owns process exit and stderr reporting.
- `src/cli.rs` owns the stable subcommand namespace.
- `src/error.rs` owns recoverable application failures.
- `src/variables.rs` keeps candidate extraction, deduplication, and citation
  resolution separate from CSV, HTML, and filesystem adapters.
- `src/qa.rs` keeps finding and verdict calculation explicit while isolating CSV,
  filesystem, checksum, URL, and Markdown report effects at the command boundary.
- `src/retrieval.rs` flattens canonical artifacts into typed records, while SQLite and
  filesystem adapters own FTS5 persistence, optional deterministic embedding blobs,
  and query execution.
- `src/agent_context.rs` formats citation-bearing retrieval results without changing
  retrieval ranking, SQLite persistence, or source artifact schemas.
- `src/mcp.rs` exposes read-only retrieval and context tools over line-delimited
  stdio JSON-RPC and records local lifecycle state without changing retrieval behavior.
- `src/mcp_setup.rs` updates local client config files while preserving unrelated JSON/TOML content.
- `src/integration.rs` provides downstream availability, crosswalk, context formatting,
  and caveat scanning helpers over existing metadata and retrieval artifacts.
- `src/evaluation.rs` computes deterministic retrieval usefulness metrics and report
  output over retrieval and agent-context results without changing index or ranking behavior.
- `src/progress.rs` writes progress events at preservation edges and summarizes
  existing progress JSONL logs without changing producer behavior.
- Future modules must preserve the same separation of pure transforms and I/O.

Last Reviewed: 2026-06-22
Status: Verified

## Data Flow

The intended durable flow is:

```text
source discovery -> raw archive -> metadata/chunks -> variables -> QA -> SQLite index -> retrieval -> agent context/evaluation/MCP/integration
                       \-> progress logs -> progress summary
```

CSV and JSONL artifacts remain canonical interchange formats. SQLite is a
rebuildable serving artifact. Every derived record must retain source URL and
available local document, page, chunk, and checksum lineage.

Last Reviewed: 2026-06-22
Status: Verified

## Constraints

- The public program is `rkb`; behavior is selected through subcommands.
- Core functions do not read files, environment, time, network, or process state.
- Expected absence uses `Option`; recoverable failure uses typed `Result` values.
- Invalid domain states should be unrepresentable where practical.
- No `unsafe` code is permitted without an explicit architectural revision.
- Concurrency is introduced only after sequential behavior and rate limits are tested.
- New dependencies require a concrete slice and a documented reason.
- The `regex` dependency is limited to variable candidate, year, and alias recognition.
- Bundled SQLite is used so the rebuildable serving index has consistent FTS5 support.
- Hybrid retrieval falls back to lexical results unless the optional embedding table is present.
- Deterministic local embedding vectors are a reviewable serving artifact; replacing them
  with a model-backed runtime requires a separate dependency and performance review.
- Python parity is defined by tests and fixtures, not by translating implementation structure.

Last Reviewed: 2026-06-22
Status: Verified
