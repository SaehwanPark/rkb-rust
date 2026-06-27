# RKB Rust

`rkb-rust` is the test-driven Rust rewrite of the ResDAC/CMS documentation
knowledge base. It will preserve public documentation, derive traceable
metadata, and expose citation-backed retrieval through one `rkb` executable.

The repository currently contains verified rewrite slices through preservation,
metadata extraction, parsing, variables, QA, lexical and hybrid retrieval,
agent-context formatting, MCP serving/setup, retrieval evaluation, progress
summaries, and downstream integration helpers.

## Install

```bash
cargo install rkb-rust
brew install SaehwanPark/tap/rkb-rust
```

For a new-user overview with package-page-safe links, see
[docs/PROJECT_DESCRIPTION.md](docs/PROJECT_DESCRIPTION.md). For release
operations, see [docs/RELEASE.md](docs/RELEASE.md).

## Development

The Rust toolchain is pinned by `rust-toolchain.toml`.

```bash
scripts/release-check
```

Inspect the CLI contract with:

```bash
cargo run -- --help
cargo run -- agent-context --query BENE_ID
cargo run -- evaluate --sample-size 5
cargo run -- progress
cargo run -- mcp
cargo run -- integration availability --dataset carrier-ffs
```

Build the SQLite index with `cargo run -- index` before running search or
agent-context commands against local artifacts. Use `cargo run -- index
--build-embeddings` and `cargo run -- search --hybrid ...` for deterministic
hybrid reranking over the local embedding table.

## Project State

- [USER_MANUAL.md](docs/USER_MANUAL.md) is the step-by-step user guide for running the pipelines.
- [SPEC.md](SPEC.md) tracks past, present, and future capabilities.
- [ARCHITECTURE.md](ARCHITECTURE.md) records boundaries and invariants.
- [ROADMAP.md](ROADMAP.md) orders the rewrite into thin vertical slices.
- [docs/python-baseline.md](docs/python-baseline.md) pins the compatibility source.
- [docs/harness/rkb-rewrite/team-spec.md](docs/harness/rkb-rewrite/team-spec.md)
  defines the portable agent workflow.

The project handles public CMS documentation only. It does not store CMS
restricted data or protected health information.
