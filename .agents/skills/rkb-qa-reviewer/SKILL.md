---
name: rkb-qa-reviewer
description: Review a Rust rewrite slice for Python parity, typed boundaries, provenance, and cross-artifact coherence.
---

# RKB QA Reviewer

## When to Use

- Use after one rewrite slice is implemented and before it is declared complete.
- Use when producer and consumer artifact schemas may disagree.
- Do not use as a substitute for the three independent code-review passes.

## Required Inputs

- Original request and all prior `_workspace/` handoffs.
- Rust diff, tests, SDD documents, and check output.
- Relevant Python baseline evidence and both sides of changed boundaries.

## Workflow

1. Compare the request, parity contract, test spec, implementation report, and diff.
2. Compare producer output to consumer parsing field by field, including nullability and ordering.
3. Verify source URL, local document, page, chunk, and checksum provenance where applicable.
4. Review `Result`/`Option` use, state visibility, side-effect isolation, and panic paths.
5. Verify CLI output and errors are deterministic and documented.
6. Classify the result as `pass`, `fix`, or `redo` with concrete evidence.
7. Write `_workspace/04_qa_review.md`; cap local fix/review retries at two before escalation.

## Outputs

- `_workspace/04_qa_review.md` with verdict, findings, evidence, and smallest fix paths.

## Validation

- Read both sides of every changed artifact or API boundary.
- Distinguish confirmed defects from unverified behavior.
- Require Critical and High findings to be fixed or explicitly accepted before PR readiness.
- Confirm the SDD documents describe the code that actually exists.

## Stop Conditions

- Return `redo` if the parity contract or test surface is directionally incomplete.
- Stop if provenance gaps are silently filled or source evidence is missing.
- Escalate after two revision cycles or when a fix changes approved scope.
