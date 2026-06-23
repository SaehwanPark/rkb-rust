# CHANGELOG

All notable changes are recorded here using a simplified Keep a Changelog format.

## Unreleased

### Fixed

- Archive workspace retry guidance now shows `rkb archive --retry-failed-only ...`
  instead of the Python `cms-kb-archive` entry point, preventing top-level clap
  "unexpected argument" errors when operators copy retry instructions.
- Expanded `tests/cli_contract.rs` to cover all 14 implemented commands and
  preservation flag parsing smoke tests.

### Changed

- Added clap help text for CLI flags across all `rkb` subcommands.

### Added

- CLI subcommand `rkb mcp` for read-only stdio JSON-RPC tool calls, plus deterministic `start`, `status`, and `stop` state tracking.
- CLI subcommand `rkb mcp-setup` for JSON/TOML MCP client configuration updates with dry-run support.
- CLI subcommand `rkb integration` with availability, variable crosswalk, cohort dictionary, context formatting, and caveat scanning helpers.
- Optional deterministic embedding table construction and `--hybrid` search/agent-context reranking flags.
- CLI subcommand `rkb evaluate` for deterministic variable retrieval usefulness checks and benchmark report generation.
- CLI subcommand `rkb progress` for deterministic text and JSON summaries over inventory/archive progress JSONL logs.
- CLI subcommand `rkb agent-context` for deterministic, citation-preserving text and JSON context over lexical search results.
- CLI subcommands `rkb index` and `rkb search` for atomic SQLite FTS5 indexing and deterministic lexical retrieval.
- Typed retrieval records and citation-bearing text/JSON search results with exact identifier boosts.
- CLI subcommand integration for `rkb qa` with Python-compatible artifact path options.
- Deterministic provenance validation across metadata, manifests, graph records, local evidence,
  checksums, URLs, identifiers, and cross-artifact references.
- Typed QA findings and pass/fix/redo verdicts with `_workspace/06_qa_review.md` reporting.
- CLI subcommand integration for `rkb variables` with configurable chunk, manifest, metadata, graph, and workspace paths.
- Deterministic variable definition extraction, source-priority deduplication, alias/year capture, and containment edges.
- Canonical ResDAC variable-page parsing, dataset relationships, and citation resolution to archived evidence.
- Python-compatible variable CSV artifacts and `_workspace/07_variable_pack.md` partial-failure summary.
- CLI subcommand integration for `rkb parse` mapping user arguments to parsing config.
- Document parsing engine supporting HTML (using `scraper` without boilerplate), PDF (using `pdf-extract` page-by-page), and XLSX (custom OpenXML ZIP/XML parser).
- Word-boundary aligned sliding window text chunker (`chunk_text`) implementing normalizations, lookbacks, and start alignments.
- JSON serialization for chunk metadata and unified `chunks.jsonl` output stream.
- Workspace summary pack output to `_workspace/05_parsing_pack.md`.
- Comprehensive integration and unit tests for document parsing and text chunking.
- Subcommand integration for `rkb extract` mapping CLI arguments to extraction config.
- Metadata extraction engine implementing HTML scraping (using CSS selectors via `scraper`), validation of files (checksum/existence), document ID hashing (10-char SHA-1 URL prefix), and parent-child relations.
- CSV serialization for datasets, documents, document edges, ontology nodes, and ontology edges.
- Workspace summary pack output to `_workspace/04_extraction_pack.md`.
- Comprehensive integration and unit tests for the metadata extraction pipeline.
- Crawling pipeline to discover ResDAC listing pages, classify resource kinds, and execute HTTP HEAD probes.
- Preservation downloader saving files atomically, verifying SHA-256 digests, and recording manifests.
- Rate limiter handling HTTP 429 status codes with request delay, rate limit cooldowns, and circuit-breaking.
- CLI subcommand integration for `rkb inventory` and `rkb archive` mapping user arguments.
- Standard output progress rollups and machine-readable JSONL progress logging.
- Pinned integration/unit tests utilizing hermetic mock handlers for crawlers and downloaders.
- Typed domain configuration structures with custom field validation.
- Serialization and deserialization schemas for 12 domain CSV/JSONL records.
- Path resolution helpers for packaged assets with filesystem fallback.
- Domain error variants (validation, resolution, parsing) extended in `AppError`.
- Comprehensive test coverage for configuration bounds and baseline fixture roundtrips.
- Initial Rust package, library, and single `rkb` executable.
- Reserved CLI namespace for the planned rewrite commands.
- Typed unavailable-command behavior and executable contract tests.
- Pinned Python baseline fixtures with checksum provenance.
- Canonical SDD documents and portable rewrite agent harness.
- Formatting, lint, test, documentation, and CI configuration.
