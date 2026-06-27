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
scripts/release-check
scripts/release-package
scripts/release-plan
```

Focused evidence should include:

- `cargo test --test mcp`
- `cargo test --test mcp_setup`
- `cargo test --test integration`
- `cargo test --test hybrid_retrieval`
- `cargo test --test cli_contract`
- `cargo package --locked --list`
- `cargo publish --dry-run --locked`

## Performance And Release Caveats

- Hybrid reranking currently uses deterministic local vectors for reviewable,
  hermetic tests. Replacing this with a model-backed runtime requires a separate
  dependency, model download, latency, and binary-size review.
- `rkb mcp start/status/stop` records deterministic local lifecycle state; the
  foreground stdio server is the verified serving path.
- Packaging, release tagging, and publication are intentionally not performed by
  this evidence document.
- Release scripts read `release.toml` so future packaging updates can reuse the
  same validation path instead of copying commands from this document.
