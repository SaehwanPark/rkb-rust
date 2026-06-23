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

### Metadata Extraction (Dataset details, document mapping, ontology graph edges)

Status: Verified

Ported dataset parser to scrape program type, category, and availability. Ported document parser to associate documentation and assets to parent datasets, construct stable document IDs, and serialize to datasets, documents, document edges, and ontology CSV listings.

Verification:
- CLI subcommand `extract` is wired and verified.
- Integration tests verify HTML scraper parsing, file existence checks, and SHA-256 verification failures.
- Output formats, sorting, deduplication, and workspace summary output verified.
- Formatting, Clippy, and tests pass.

### Document Parsing and Chunking (HTML, PDF, and XLSX text extraction, sliding window chunker)

Status: Verified

Ported document parsing pipeline for HTML (using scraper), PDF (using pdf-extract), and XLSX (custom OpenXML zip/XML reader) page-by-page. Ported the sliding window word-boundary-aligned chunker to divide extracted text into overlapping chunks, outputting JSON records, unified JSONL stream, and workspace summary logs.

Verification:
- CLI subcommand `parse` is wired and verified.
- Integration tests verify raw text files, chunks JSON/JSONL output, workspace pack summaries, and pipeline failure modes (missing files, empty content, invalid IDs).
- Unit tests verify whitespace normalizations, end-of-chunk word boundary lookbacks, and overlap alignments.
- Formatting, Clippy, and tests pass.

### Variable-Level Metadata and Canonical Variable Extraction

Status: Verified

Ported conservative variable definition extraction from parsed chunks, source-priority
deduplication, archived ResDAC variable-page parsing, dataset containment edges, and
canonical citation resolution.

Verification:

- CLI subcommand `variables` is wired with Python-compatible path options.
- Unit and integration tests verify definition evidence, aliases, years, stable IDs,
  source priority, canonical HTML parsing, deterministic CSV schemas, and partial failures.
- Python variable tests, formatting, Clippy, Rust tests, documentation, and fixture checks pass.

### Provenance QA

Status: Verified

Ported required and optional artifact loading, SHA-256 and local evidence checks, URL and
archive-manifest validation, identifier and cross-artifact reference checks, deterministic
findings, pass/fix/redo verdicts, and workspace reporting.

Verification:

- CLI subcommand `qa` is wired with Python-compatible artifact path options.
- Focused tests verify valid provenance, bounded integrity failures, fatal missing inputs,
  report generation, counters, and command failure status.
- Python QA tests, formatting, Clippy, Rust tests, and documentation pass.

### SQLite FTS5 Indexing and Deterministic Lexical Retrieval

Status: Verified

Ported canonical record flattening, atomic SQLite FTS5 index construction, exact-term lexical
boosting, deterministic result ordering, citation-bearing snippets, and text/JSON CLI output.

Verification:

- CLI subcommands `index` and `search` are wired with Python-compatible path and query options.
- Focused tests verify required and optional inputs, schema failures, FTS index rebuilds,
  identifier and chunk ranking, citations, query validation, and JSON output.
- Python retrieval tests, formatting, Clippy, Rust tests, documentation, and fixture checks pass.

### Agent Context Formatting

Status: Verified

Ported agent-oriented context formatting over verified lexical retrieval results, with stable
citation markers, provenance fields, text output, and JSON output.

Verification:

- CLI subcommand `agent-context` is wired with search-compatible path, query, limit, and JSON options.
- Focused tests verify deterministic citation markers, empty-result output, JSON shape,
  command execution, and retrieval validation error propagation.
- Formatting, Clippy, Rust tests, and documentation pass.

### Progress Summary

Status: Verified

Ported a deterministic progress-log reader for existing inventory and archive JSONL events,
with text and JSON CLI summaries over stages, event types, latest event, and latest counts.

Verification:

- CLI subcommand `progress` is wired with repeatable `--log` path options and `--json`.
- Focused tests verify deterministic text, JSON shape, empty logs, missing logs,
  malformed JSONL line reporting, and reserved-command coherence.
- Formatting, Clippy, Rust tests, and documentation pass.

## Present

No production rewrite slice is active.

## Future

- Evaluate semantic reranking only after lexical parity is verified.
- Port MCP serving, evaluation, and integration helpers.
