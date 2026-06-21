---
name: rkb-rewrite-orchestrator
description: Coordinate one test-first Python-to-Rust parity slice with deterministic handoffs and bounded review.
---

# RKB Rewrite Orchestrator

## When to Use

- Use for any production behavior ported from the Python knowledge base.
- Use when a slice changes a reserved `rkb` command or an artifact contract.
- Do not use for documentation-only corrections with no behavior change.

## Required Inputs

- Requested behavior and acceptance criteria.
- Pinned Python baseline and relevant source modules or fixtures.
- Current `SPEC.md`, `ARCHITECTURE.md`, and tests.

## Workflow

1. Record scope, branch, acceptance criteria, and non-goals in `_workspace/00_request.md`.
2. Invoke `rkb-parity-analyst` to produce `_workspace/01_parity_contract.md`.
3. Have `rkb-rust-porter` write `_workspace/02_test_spec.md` and failing tests before implementation.
4. Commit the test contract separately when it is stable.
5. Implement the smallest complete vertical slice and write `_workspace/03_implementation_report.md`.
6. Update SDD documents to match actual behavior.
7. Invoke `rkb-qa-reviewer` for `_workspace/04_qa_review.md` and resolve blocking findings.
8. Run formatting, Clippy, tests, documentation, fixture checks, and the preferred three-pass review loop.
9. Push the feature branch and open a PR only after local checks pass.

## Outputs

- The five deterministic `_workspace/` handoffs named above.
- Focused tests, the smallest complete Rust implementation, and synchronized SDD documents.
- A PR report listing files changed, checks run, deviations, and unresolved risks.

## Validation

- Confirm the branch is not `main` after the bootstrap release.
- Confirm a failing test existed before production behavior changed.
- Confirm Rust output is compared to Python behavior at every changed boundary.
- Confirm no unrelated command, dependency, or artifact format changed.

## Stop Conditions

- Stop if the Python baseline is ambiguous or cannot produce evidence.
- Stop if the slice requires an unapproved artifact or public CLI incompatibility.
- Stop if implementation expands beyond one reviewable vertical slice.
- Stop if Critical or High findings remain unresolved.
