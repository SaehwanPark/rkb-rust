# Release Evidence

This draft records the local evidence required before publishing or tagging a
public release. It is not a release announcement.

## Compatibility

- Public executable remains `rkb`.
- Implemented commands: `inventory`, `archive`, `extract`, `parse`, `variables`,
  `qa`, `index`, `search`, `agent-context`, `mcp`, `mcp-setup`, `evaluate`,
  `progress`, and `integration`.
- Canonical CSV and JSONL producer schemas remain unchanged.
- SQLite remains a rebuildable serving artifact; `record_embeddings` is optional
  and created only with `rkb index --build-embeddings`.

## Verification Commands

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo doc --no-deps
```

Focused evidence should include:

- `cargo test --test mcp`
- `cargo test --test mcp_setup`
- `cargo test --test integration`
- `cargo test --test hybrid_retrieval`
- `cargo test --test cli_contract`

## Performance And Release Caveats

- Hybrid reranking currently uses deterministic local vectors for reviewable,
  hermetic tests. Replacing this with a model-backed runtime requires a separate
  dependency, model download, latency, and binary-size review.
- `rkb mcp start/status/stop` records deterministic local lifecycle state; the
  foreground stdio server is the verified serving path.
- Packaging, release tagging, and publication are intentionally not performed by
  this evidence document.
