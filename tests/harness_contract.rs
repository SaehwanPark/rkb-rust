use std::fs;
use std::path::{Path, PathBuf};

const SKILLS: &[&str] = &[
  "rkb-rewrite-orchestrator",
  "rkb-parity-analyst",
  "rkb-rust-porter",
  "rkb-qa-reviewer",
];

const REQUIRED_SECTIONS: &[&str] = &[
  "## When to Use",
  "## Required Inputs",
  "## Workflow",
  "## Outputs",
  "## Validation",
  "## Stop Conditions",
];

fn root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
  fs::read_to_string(path)
    .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn skills_have_portable_contracts() {
  for skill in SKILLS {
    let path = root().join(format!(".agents/skills/{skill}/SKILL.md"));
    let text = read(&path);

    assert!(text.starts_with("---\n"), "{skill} lacks frontmatter");
    assert!(text.contains(&format!("name: {skill}")));
    assert!(text.contains("description:"));
    for section in REQUIRED_SECTIONS {
      assert!(text.contains(section), "{skill} lacks {section}");
    }
  }
}

#[test]
fn team_spec_names_roles_and_handoffs() {
  let text = read(&root().join("docs/harness/rkb-rewrite/team-spec.md"));

  for skill in SKILLS {
    assert!(text.contains(skill), "team spec omitted {skill}");
  }
  for handoff in [
    "_workspace/00_request.md",
    "_workspace/01_parity_contract.md",
    "_workspace/02_test_spec.md",
    "_workspace/03_implementation_report.md",
    "_workspace/04_qa_review.md",
  ] {
    assert!(text.contains(handoff), "team spec omitted {handoff}");
  }
}

#[test]
fn agents_guide_stays_concise() {
  let text = read(&root().join("AGENTS.md"));
  assert!(
    text.lines().count() <= 60,
    "AGENTS.md should remain concise"
  );
  for heading in ["## What", "## Why", "## How"] {
    assert!(text.contains(heading), "AGENTS.md omitted {heading}");
  }
}

#[test]
fn fixture_manifest_matches_checksum_ledger() {
  let fixture_root = root().join("tests/fixtures/python-baseline");
  let manifest = read(&fixture_root.join("manifest.json"));
  let checksums = read(&fixture_root.join("checksums.sha256"));

  for line in checksums.lines() {
    let (hash, path) = line
      .split_once("  ")
      .expect("checksum entries should use sha256sum format");
    assert!(manifest.contains(hash), "manifest omitted hash for {path}");
    assert!(
      manifest.contains(&format!("\"path\": \"{path}\"")),
      "manifest omitted {path}"
    );
  }
}
