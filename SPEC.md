# SPEC

## Past

### Repository foundation and rewrite harness

Status: Verified

The repository provides a pinned Rust toolchain, one `rkb` executable, a typed
reserved-command contract, representative Python parity fixtures, canonical SDD
documents, a portable rewrite harness, and CI validation.

Verification:

- `rkb --help` lists every reserved command.
- Reserved commands return deterministic typed unavailable errors.
- Formatting, Clippy, tests, documentation, and harness-contract checks pass.

## Present

No production rewrite slice is active.

## Future

- Port typed record schemas and validated configuration boundaries.
- Port inventory discovery and archive preservation.
- Port metadata extraction and document parsing.
- Port variable extraction and provenance QA.
- Port SQLite FTS5 indexing and deterministic retrieval.
- Port agent context, MCP serving, evaluation, progress, and integration helpers.
- Evaluate semantic reranking only after lexical parity is verified.
