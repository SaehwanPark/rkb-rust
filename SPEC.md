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

### Record schemas and validated configuration boundaries

Status: Verified

Ported 9 domain configuration structures with custom validators, path resolution helper, 12 domain record structures (CSV/JSONL compatible), and extended typed application error variants.

Verification:

- Unit and integration tests verify invalid configurations are rejected.
- Site inventory, archive manifest, and chunks JSONL baseline fixtures are successfully parsed (roundtrip checks).
- Clippy, formatting, and tests pass.

### Preservation (Inventory Crawl, Archive Downloader, Rate Limiting, Progress Logs)

Status: Verified

Ported crawler to discover listing pages, classify resource kinds, execute HTTP probes, and record inventory CSV rows/edges. Ported archiver to download resources under structured directories, verify SHA-256 integrity, write manifests, and implement HTTP 429 rate limit backoff and circuit-breaking.

Verification:
- CLI subcommands `inventory` and `archive` are wired and verified.
- Integration/unit tests run hermetically using mock closures for crawling, downloading, and sleeping.
- Output formats, schema fields, sorting, and rate-limiting limits verified.
- Formatting, Clippy, and tests pass.

## Present

No production rewrite slice is active.

## Future

- Port metadata extraction and document parsing.
- Port variable extraction and provenance QA.
- Port SQLite FTS5 indexing and deterministic retrieval.
- Port agent context, MCP serving, evaluation, progress, and integration helpers.
- Evaluate semantic reranking only after lexical parity is verified.
