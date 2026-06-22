# RKB Rust

`rkb-rust` is the test-driven Rust rewrite of the ResDAC/CMS documentation
knowledge base. It will preserve public documentation, derive traceable
metadata, and expose citation-backed retrieval through one `rkb` executable.

The repository currently contains the rewrite foundation. Command names are
reserved, but production pipeline behavior has not been ported yet.

## Development

The Rust toolchain is pinned by `rust-toolchain.toml`.

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo doc --no-deps
```

Inspect the CLI contract with:

```bash
cargo run -- --help
cargo run -- index
```

The second command intentionally exits with status 1 until the indexing slice
passes parity review.

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
