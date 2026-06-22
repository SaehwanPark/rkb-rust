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

## Present

### Variable-level metadata and canonical variable extraction

Status: Active
Started: 2026-06-22
Branch: `feat/variable-extraction`

Port chunk-based variable definition extraction and archived ResDAC variable-page
parsing into `rkb variables`. Produce deterministic variable catalogs, containment
edges, canonical variable records, citation resolution, and a workspace summary.

Verification:

- Focused parity tests cover extraction, deduplication, canonical pages, and failures.
- `rkb variables` writes the four Python-compatible CSV artifacts deterministically.
- Formatting, Clippy, tests, documentation, and fixture checks pass.

Out of scope:

- QA, retrieval indexing, search, semantic reranking, and concurrency.

## Future

- Port provenance QA.
- Port SQLite FTS5 indexing and deterministic retrieval.
- Port agent context, MCP serving, evaluation, progress, and integration helpers.
- Evaluate semantic reranking only after lexical parity is verified.
