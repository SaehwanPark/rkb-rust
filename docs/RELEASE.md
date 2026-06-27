# Release Runbook

This runbook keeps crates.io, GitHub Release, and Homebrew tap publication
repeatable. It documents the release path only; do not publish without explicit
maintainer approval.

## Configuration

Release-specific values live in [`release.toml`](https://github.com/SaehwanPark/rkb-rust/blob/main/release.toml).
Cargo package metadata remains in [`Cargo.toml`](https://github.com/SaehwanPark/rkb-rust/blob/main/Cargo.toml).

Current distribution targets:

- crates.io package: `rkb-rust`
- installed binary: `rkb`
- repository: <https://github.com/SaehwanPark/rkb-rust>
- Homebrew tap: `SaehwanPark/homebrew-tap`
- Homebrew install command: `brew install SaehwanPark/tap/rkb-rust`

## Required Secrets

Add these GitHub Actions secrets to the project repository before publishing:

- `CRATES_IO_TOKEN`: crates.io API token allowed to publish `rkb-rust`.
- `HOMEBREW_TAP_TOKEN`: GitHub token with write access to
  `SaehwanPark/homebrew-tap`.

Never commit token values to this repository or to the tap.

## Local Preflight

Run the reusable scripts from the repository root:

```bash
scripts/release-check
scripts/release-package
scripts/release-plan
```

`scripts/release-check` runs formatting, Clippy, tests, and docs.
`scripts/release-package` verifies Cargo package contents and runs
`cargo publish --dry-run --locked`.
`scripts/release-plan` runs `dist plan` and prints the configured release
targets.

For a local artifact build before tagging:

```bash
scripts/release-plan --build
```

## Package Contents

The Cargo package uses an explicit allowlist in
[`Cargo.toml`](https://github.com/SaehwanPark/rkb-rust/blob/main/Cargo.toml).
Before publishing, confirm `scripts/release-package` does not include archived
runtime data such as `data/`, `manifests/`, `_workspace/`, `.agents/`, or
`target/`.

## Publishing

Publishing is tag-driven. After all local checks pass and the release version is
approved:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow validates the package, publishes to crates.io, builds
release artifacts with `dist`, creates the GitHub Release, and updates the
Homebrew tap.

## Post-Release Smoke Tests

Verify both public install paths:

```bash
cargo install rkb-rust
rkb --version
brew install SaehwanPark/tap/rkb-rust
rkb --version
```

Confirm the package pages and release notes link to:

- <https://github.com/SaehwanPark/rkb-rust>
- <https://github.com/SaehwanPark/rkb-rust/blob/main/docs/PROJECT_DESCRIPTION.md>
- <https://github.com/SaehwanPark/rkb-rust/blob/main/docs/USER_MANUAL.md>
- <https://github.com/SaehwanPark/rkb-rust/blob/main/CHANGELOG.md>
