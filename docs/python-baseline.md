# Python Compatibility Baseline

The Rust rewrite treats observable Python behavior as its compatibility source,
not Python module structure.

## Pinned source

- Repository: <https://github.com/SaehwanPark/resdac-knowledge-base>
- Git commit: `d1120cc73367894fb15dc6f44fe3ba452d3e2e10`
- PyPI package: `resdac-knowledge-base==1.0.2`
- PyPI release: <https://pypi.org/project/resdac-knowledge-base/1.0.2/>
- Baseline captured: 2026-06-21

The full source corpus is deliberately not duplicated here. Representative
outputs live under `tests/fixtures/python-baseline/`; `manifest.json` records
their source and hashes.

## Compatibility policy

- Preserve artifact schemas, stable identifiers, ordering, citations, and error
  meaning unless an approved spec explicitly records a divergence.
- Compare behavior using focused fixtures and bounded Python commands.
- Do not require the Python repository or Python runtime in normal Rust CI.
- Add a fixture only when it protects a concrete porting contract.
- Regenerate fixtures from the pinned source or record a deliberate baseline update.

Scores and performance measurements are evidence, not permanent wire contracts,
unless a slice explicitly promotes them into acceptance criteria.
