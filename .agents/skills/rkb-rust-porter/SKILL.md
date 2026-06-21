---
name: rkb-rust-porter
description: Port one Python behavior slice into idiomatic functional-first Rust using tests as the executable contract.
---

# RKB Rust Porter

## When to Use

- Use after a parity contract exists for one bounded rewrite slice.
- Use to replace one reserved-command route with complete tested behavior.
- Do not use for broad module-by-module translation without a vertical acceptance test.

## Required Inputs

- `_workspace/00_request.md` and `_workspace/01_parity_contract.md`.
- Relevant Python tests, Rust architecture constraints, and fixtures.
- An isolated feature branch.

## Workflow

1. Write `_workspace/02_test_spec.md` with concrete behavior and failure cases.
2. Add focused Rust tests and run them to capture the expected failure.
3. Commit the test contract separately when requested by the orchestrator.
4. Model domain concepts with structs/enums and recoverable failure with `Result`.
5. Implement pure validation and transforms before thin I/O adapters.
6. Keep mutation local and avoid hidden state, globals, wall-clock reads, and environment reads in the core.
7. Document public contracts and only comment rationale, invariants, or non-obvious tradeoffs.
8. Run focused tests, then full formatting, lint, test, and documentation checks.
9. Write `_workspace/03_implementation_report.md` with evidence and deviations.

## Outputs

- `_workspace/02_test_spec.md` and `_workspace/03_implementation_report.md`.
- Passing tests and a complete implementation for one vertical slice.
- Updated `SPEC.md`, `ARCHITECTURE.md`, and `CHANGELOG.md` when applicable.

## Validation

- State and effects are visible in signatures and module boundaries.
- Tests assert behavior rather than implementation details.
- Command output, exit status, ordering, and provenance match the parity contract.
- No placeholder path remains for behavior claimed complete.

## Stop Conditions

- Stop if lifetimes or shared mutation indicate the data flow is not understood.
- Stop before adding `unsafe`, concurrency, or a broad abstraction not required by the slice.
- Stop if unrelated tests fail or more than one command must change unexpectedly.
