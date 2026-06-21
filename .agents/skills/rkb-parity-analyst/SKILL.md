---
name: rkb-parity-analyst
description: Derive a bounded observable-behavior contract and fixtures from the pinned Python implementation.
---

# RKB Parity Analyst

## When to Use

- Use before porting a Python command, model, transform, or artifact boundary.
- Use when a Rust result differs from the pinned Python baseline.
- Do not use to prescribe a line-by-line translation of Python internals.

## Required Inputs

- `_workspace/00_request.md`.
- `docs/python-baseline.md` and relevant Python source/tests.
- Existing Rust fixtures and compatibility decisions.

## Workflow

1. Identify observable inputs, outputs, ordering, identifiers, provenance, and failures.
2. Read Python tests and execute the smallest deterministic baseline commands needed.
3. Separate required compatibility from accidental Python implementation details.
4. Select minimal fixtures with no restricted data or unnecessary corpus content.
5. Record expected behavior, explicit divergences, edge cases, and fixture provenance.
6. Write `_workspace/01_parity_contract.md` without editing Rust production code.

## Outputs

- `_workspace/01_parity_contract.md`.
- Small checksummed fixtures under `tests/fixtures/python-baseline/` when needed.

## Validation

- Every expected output cites a Python test, command, or artifact.
- Fixtures include source commit, generation command, and checksum.
- Contracts cover success, invalid input, absence, and deterministic ordering where applicable.

## Stop Conditions

- Stop if source behavior changes across repeated runs without an explained input difference.
- Stop if fixture generation would copy restricted, sensitive, or large corpus data.
- Stop if compatibility requires a product decision not recorded in the request.
