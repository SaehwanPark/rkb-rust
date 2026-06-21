# CHANGELOG

All notable changes are recorded here using a simplified Keep a Changelog format.

## Unreleased

### Added

- Typed domain configuration structures with custom field validation.
- Serialization and deserialization schemas for 12 domain CSV/JSONL records.
- Path resolution helpers for packaged assets with filesystem fallback.
- Domain error variants (validation, resolution, parsing) extended in `AppError`.
- Comprehensive test coverage for configuration bounds and baseline fixture roundtrips.
- Initial Rust package, library, and single `rkb` executable.
- Reserved CLI namespace for the planned rewrite commands.
- Typed unavailable-command behavior and executable contract tests.
- Pinned Python baseline fixtures with checksum provenance.
- Canonical SDD documents and portable rewrite agent harness.
- Formatting, lint, test, documentation, and CI configuration.
