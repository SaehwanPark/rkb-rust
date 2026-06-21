# RKB Rewrite Harness Team Spec

## Goal

Port one observable Python behavior slice at a time into idiomatic Rust while
preserving provenance, deterministic artifacts, and reviewable evidence. The
harness is repository-local and works through files rather than runtime-specific
agent messaging.

## Pattern

The outer pattern is a sequential Pipeline. The final implementation-to-QA edge
uses Producer-Reviewer with no more than two local revision cycles.

## Roles

| Role | Responsibility | Writes |
| --- | --- | --- |
| `rkb-rewrite-orchestrator` | Scope, sequencing, branch workflow, and final handoff | `_workspace/00_request.md` |
| `rkb-parity-analyst` | Observable Python compatibility contract and fixtures | `_workspace/01_parity_contract.md` |
| `rkb-rust-porter` | Failing tests followed by the smallest complete Rust slice | `_workspace/02_test_spec.md`, `_workspace/03_implementation_report.md` |
| `rkb-qa-reviewer` | Cross-boundary parity, provenance, and architecture review | `_workspace/04_qa_review.md` |

## Phase Order

1. Scope: record acceptance criteria, non-goals, source baseline, and branch.
2. Analyze: capture observable Python behavior and fixture provenance.
3. Specify: write failing Rust tests and record the test contract.
4. Implement: complete one vertical slice and synchronize SDD documents.
5. Review: compare both sides of every changed boundary and issue pass/fix/redo.
6. Publish: run full checks, three review passes, push, and open a PR.

## Handoff Contract

- `_workspace/00_request.md`: request, branch, acceptance criteria, and non-goals.
- `_workspace/01_parity_contract.md`: Python evidence, cases, schemas, and divergences.
- `_workspace/02_test_spec.md`: failing tests and expected outcomes.
- `_workspace/03_implementation_report.md`: files, checks, deviations, and risks.
- `_workspace/04_qa_review.md`: pass/fix/redo verdict and boundary evidence.

Generated handoffs remain local. Durable decisions must also be reflected in
tests, canonical SDD documents, and the pull request.

## Failure Policy

- Missing or unstable baseline evidence stops parity analysis.
- A local implementation defect returns `fix`; an incomplete contract returns `redo`.
- Two failed revision cycles require user escalation.
- Unrelated test failures are reported and not repaired opportunistically.
- No role may infer or fabricate missing provenance.

## Portability Rules

- Skills live in `.agents/skills/` with standard YAML frontmatter.
- No model pin, agent SDK, MCP orchestrator, or peer-to-peer agent runtime is required.
- Coordination uses Git, Markdown, tests, and deterministic files.
- Model-specific retries may be added only in clearly removable reference notes.

## Validation Scenarios

Normal flow: port one record parser, capture Python fixtures, observe failing Rust
tests, implement a pure parser, pass QA, and open a feature PR.

Failure flow: if Python emits inconsistent output or fixture provenance is
missing, stop after parity analysis and report the unverified contract; do not
write production Rust behavior.
