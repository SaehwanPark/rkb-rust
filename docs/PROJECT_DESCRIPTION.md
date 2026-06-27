# RKB Rust

`rkb-rust` installs the `rkb` command-line tool for building a local,
traceable knowledge base from public ResDAC and CMS documentation.

RKB is for researchers, analysts, and software agents who need to preserve
public documentation, derive metadata, index text, and retrieve citation-backed
answers from local artifacts. It handles public documentation only. It does not
store CMS restricted data, claims data, credentials, or protected health
information.

## Install

Install from crates.io:

```bash
cargo install rkb-rust
```

Install from the Homebrew tap:

```bash
brew install SaehwanPark/tap/rkb-rust
```

Both installation methods provide the same executable:

```bash
rkb --help
rkb --version
```

## First Commands

Create a local inventory of public documentation:

```bash
rkb inventory
```

Download public source documents from the inventory:

```bash
rkb archive
```

Extract metadata, parse documents, and build a searchable index:

```bash
rkb extract
rkb parse
rkb variables
rkb qa
rkb index
```

Search the local index and format evidence for agents:

```bash
rkb search --query BENE_ID
rkb agent-context --query BENE_ID
```

## What RKB Produces

RKB writes durable local artifacts such as CSV manifests, JSONL chunks, QA
reports, and a rebuildable SQLite search index. Derived records are designed to
retain source URL and local evidence so results can be traced back to archived
public documentation.

## More Documentation

- Project repository: <https://github.com/SaehwanPark/rkb-rust>
- User manual: <https://github.com/SaehwanPark/rkb-rust/blob/main/docs/USER_MANUAL.md>
- Architecture notes: <https://github.com/SaehwanPark/rkb-rust/blob/main/ARCHITECTURE.md>
- Changelog: <https://github.com/SaehwanPark/rkb-rust/blob/main/CHANGELOG.md>
- Release runbook: <https://github.com/SaehwanPark/rkb-rust/blob/main/docs/RELEASE.md>
- License: <https://github.com/SaehwanPark/rkb-rust/blob/main/LICENSE>
- Issues: <https://github.com/SaehwanPark/rkb-rust/issues>
