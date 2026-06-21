# Rust Rewrite Design

## Goal

Rewrite the ResDAC/CMS documentation knowledge base as an idiomatic Rust system
without losing provenance, deterministic outputs, or agent-facing citations.
The executable surface is one program, `rkb`, with subcommands.

## Design principles

- Preserve first; derive later.
- Keep I/O at the edge and deterministic transforms in a pure core.
- Model domain concepts and failures explicitly.
- Port observable behavior through tests instead of translating Python classes.
- Prefer understandable sequential behavior before adding concurrency.
- Keep CSV and JSONL canonical; treat SQLite as rebuildable.
- Add dependencies only when a verified slice needs them.

## Command mapping

| `rkb` command | Responsibility |
| --- | --- |
| `inventory` | Discover source pages, assets, and provenance edges. |
| `archive` | Preserve raw documents with checksums and polite retries. |
| `extract` | Produce dataset, document, and graph metadata. |
| `parse` | Produce text and provenance-bearing chunks. |
| `variables` | Produce variable records and containment edges. |
| `qa` | Validate checksums, references, schemas, and provenance. |
| `index` | Build the derived SQLite retrieval index. |
| `search` | Return deterministic citation-bearing results. |
| `agent-context` | Format retrieval output for agent consumers. |
| `mcp` | Serve read-only MCP tools. |
| `mcp-setup` | Configure supported local MCP clients. |
| `evaluate` | Measure retrieval and citation quality. |
| `progress` | Summarize long-running operation events. |
| `integration` | Expose downstream research integration helpers. |

## Module direction

Each command should compose four layers:

1. A typed configuration boundary.
2. Pure validation and transformation functions.
3. Thin adapters for network, filesystem, database, clock, or process effects.
4. A CLI handler that maps typed outcomes to user-facing output and exit status.

Shared domain types may be extracted only after two real consumers demonstrate
the need. Async execution, parser libraries, SQLite bindings, MCP libraries, and
semantic runtimes remain decisions owned by their implementation slices.

## Verification strategy

Every slice follows the rewrite harness: capture Python behavior, write failing
Rust tests, implement the smallest complete behavior, compare both sides of each
artifact boundary, and update canonical SDD state. Performance work begins only
after semantic parity is demonstrated.
