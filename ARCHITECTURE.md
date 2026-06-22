# ARCHITECTURE

## Overview

`rkb-rust` is organized as one library crate and one `rkb` binary. The binary
owns process concerns; library modules own typed domain transformations and thin
side-effect adapters for preservation, extraction, parsing, variable metadata, provenance QA,
and lexical retrieval.

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
  filesystem adapters own FTS5 persistence and query execution.
- Future modules must preserve the same separation of pure transforms and I/O.

Last Reviewed: 2026-06-22
Status: Verified

## Data Flow

The intended durable flow is:

```text
source discovery -> raw archive -> metadata/chunks -> variables -> QA -> SQLite index -> retrieval
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
- Python parity is defined by tests and fixtures, not by translating implementation structure.

Last Reviewed: 2026-06-22
Status: Verified
