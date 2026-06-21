# Repository Agents Guide

Keep this file short and repo-wide. Use `SPEC.md`, `ARCHITECTURE.md`, and the
rewrite harness for task-specific detail.

## What

- Rust rewrite of the ResDAC/CMS documentation archive and retrieval toolkit.
- `rkb` is the only executable; actions are subcommands.
- Canonical code paths are `src/`, `tests/`, `docs/`, and `.agents/skills/`.
- `_workspace/` contains local, inspectable harness handoffs and is not shipped.

## Why

- Preserve public source documents before deriving metadata or retrieval data.
- Keep every derived fact traceable to source URL and archived-document evidence.
- Make ports reviewable through explicit types, pure transforms, and parity tests.

## How

- Use functional-first Rust: immutable values, pure core logic, and I/O adapters at edges.
- Use `Result` and domain error enums for recoverable failures; do not use panics as flow control.
- Write a failing parity test before implementing each rewrite slice.
- Keep public APIs documented and comments focused on rationale and invariants.
- Use 2-space indentation; run `cargo fmt --all --check`.
- Run `cargo clippy --all-targets --all-features -- -D warnings`.
- Run `cargo test --all-targets --all-features` and `cargo doc --no-deps`.
- Keep `SPEC.md`, `ARCHITECTURE.md`, and `CHANGELOG.md` aligned with implementation.
- Follow `docs/harness/rkb-rewrite/team-spec.md` for rewrite work.
- Add recurring setup or debugging traps to `LESSONS.md`.
