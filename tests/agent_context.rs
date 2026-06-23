use rkb::agent_context::{build_agent_context, format_agent_context_text};
use rkb::config::RetrievalConfig;
use rkb::retrieval::{RecordType, SearchResult, build_index};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
  root: PathBuf,
  config: RetrievalConfig,
}

impl Drop for Fixture {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.root);
  }
}

fn write(path: &Path, content: &str) {
  fs::create_dir_all(path.parent().expect("fixture path should have a parent"))
    .expect("fixture directory should be created");
  fs::write(path, content).expect("fixture should be written");
}

fn path_str(path: &Path) -> &str {
  path.to_str().expect("fixture path should be UTF-8")
}

fn fixture() -> Fixture {
  let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
  let root = std::env::temp_dir().join(format!(
    "rkb-agent-context-{}-{sequence}",
    std::process::id()
  ));
  let metadata = root.join("data/metadata");
  let parsed = root.join("data/parsed");

  let datasets = metadata.join("datasets.csv");
  write(
    &datasets,
    "dataset_id,name,program,category,availability,source_url,local_path,sha256,extraction_notes\nmbsf,Medicare Beneficiary Summary File,Medicare,Enrollment,Available,https://resdac.org/cms-data/files/mbsf,raw/mbsf.html,fake-sha,\n",
  );
  let documents = metadata.join("documents.csv");
  write(
    &documents,
    "document_id,dataset_id,title,document_kind,source_url,local_path,sha256,content_type,extraction_notes\nmbsf__codebook,mbsf,MBSF Codebook,pdf,https://resdac.org/cms-data/files/mbsf-codebook,raw/mbsf.pdf,fake-doc-sha,application/pdf,\n",
  );
  let variables = metadata.join("variables.csv");
  write(
    &variables,
    "variable_id,variable_name,dataset_id,definition,aliases,years,source_document,source_url,page,chunk_id,extraction_notes\nmbsf__var__bene-id,BENE_ID,mbsf,Beneficiary identifier used to link claims and enrollment records.,beneficiary id,2020,parsed/mbsf.txt,https://resdac.org/cms-data/files/mbsf-codebook,3,chunk-1,\n",
  );
  let chunks = parsed.join("chunks.jsonl");
  write(
    &chunks,
    "{\"chunk_id\":\"chunk-1\",\"source_document\":\"parsed/mbsf.txt\",\"page\":3,\"text\":\"Dual eligibility indicators describe Medicare and Medicaid enrollment.\",\"dataset\":\"mbsf\",\"url\":\"https://resdac.org/cms-data/files/mbsf-codebook\"}\n",
  );

  Fixture {
    config: RetrievalConfig {
      datasets_metadata_path: datasets,
      documents_metadata_path: documents,
      variables_metadata_path: variables,
      chunks_jsonl_path: chunks,
      database_path: root.join("data/index/retrieval.sqlite"),
      ..RetrievalConfig::default()
    },
    root,
  }
}

fn agent_context_command(fixture: &Fixture, query: &str, limit: &str, json: bool) -> Command {
  let mut command = Command::new(env!("CARGO_BIN_EXE_rkb"));
  command.args([
    "agent-context",
    "--query",
    query,
    "--limit",
    limit,
    "--datasets-metadata",
    path_str(&fixture.config.datasets_metadata_path),
    "--documents-metadata",
    path_str(&fixture.config.documents_metadata_path),
    "--variables-metadata",
    path_str(&fixture.config.variables_metadata_path),
    "--chunks-jsonl",
    path_str(&fixture.config.chunks_jsonl_path),
    "--database-path",
    path_str(&fixture.config.database_path),
  ]);
  if json {
    command.arg("--json");
  }
  command
}

#[test]
fn formats_context_with_deterministic_citations() {
  let context = build_agent_context(
    "BENE_ID",
    vec![SearchResult {
      record_id: "mbsf__var__bene-id".to_string(),
      record_type: RecordType::Variable,
      title: "BENE_ID".to_string(),
      dataset_id: "mbsf".to_string(),
      score: 12.345_67,
      snippet: "Beneficiary identifier used to link claims.".to_string(),
      source_url: "https://resdac.org/cms-data/files/mbsf-codebook".to_string(),
      source_document: "parsed/mbsf.txt".to_string(),
      page: Some(3),
    }],
  );

  assert_eq!(context.result_count, 1);
  assert_eq!(context.entries[0].citation, "[1]");
  let text = format_agent_context_text(&context);
  assert!(text.contains("Query: BENE_ID"));
  assert!(text.contains("[1] BENE_ID (variable) dataset=mbsf score=12.346"));
  assert!(text.contains("Source: https://resdac.org/cms-data/files/mbsf-codebook page 3"));
  assert!(text.contains("Document: parsed/mbsf.txt"));
}

#[test]
fn formats_empty_context_successfully() {
  let context = build_agent_context("unfindableterm", Vec::new());

  assert_eq!(context.result_count, 0);
  assert_eq!(
    format_agent_context_text(&context),
    "Query: unfindableterm\nNo matching context found."
  );
}

#[test]
fn agent_context_command_emits_text_and_json() {
  let fixture = fixture();
  build_index(&fixture.config).expect("index should build");

  let text = agent_context_command(&fixture, "BENE_ID", "1", false)
    .output()
    .expect("agent-context text command should execute");
  assert!(
    text.status.success(),
    "{}",
    String::from_utf8_lossy(&text.stderr)
  );
  let stdout = String::from_utf8(text.stdout).expect("text output should be UTF-8");
  assert!(stdout.contains("Query: BENE_ID"));
  assert!(stdout.contains("[1] BENE_ID (variable) dataset=mbsf"));
  assert!(stdout.contains("https://resdac.org/cms-data/files/mbsf-codebook page 3"));

  let json = agent_context_command(&fixture, "BENE_ID", "1", true)
    .output()
    .expect("agent-context JSON command should execute");
  assert!(
    json.status.success(),
    "{}",
    String::from_utf8_lossy(&json.stderr)
  );
  let payload: serde_json::Value =
    serde_json::from_slice(&json.stdout).expect("JSON output should parse");
  assert_eq!(payload["query"], "BENE_ID");
  assert_eq!(payload["result_count"], 1);
  assert_eq!(payload["entries"][0]["citation"], "[1]");
  assert_eq!(payload["entries"][0]["record_id"], "mbsf__var__bene-id");
  assert_eq!(payload["entries"][0]["record_type"], "variable");
  assert_eq!(payload["entries"][0]["title"], "BENE_ID");
  assert_eq!(payload["entries"][0]["dataset_id"], "mbsf");
  assert!(payload["entries"][0]["score"].is_number());
  assert!(
    payload["entries"][0]["snippet"]
      .as_str()
      .expect("snippet should be a string")
      .contains("Beneficiary identifier")
  );
  assert_eq!(
    payload["entries"][0]["source_url"],
    "https://resdac.org/cms-data/files/mbsf-codebook"
  );
  assert_eq!(payload["entries"][0]["source_document"], "parsed/mbsf.txt");
  assert_eq!(payload["entries"][0]["page"], 3);
}

#[test]
fn agent_context_command_handles_empty_results_and_invalid_queries() {
  let fixture = fixture();
  build_index(&fixture.config).expect("index should build");

  let empty = agent_context_command(&fixture, "unfindableterm", "5", false)
    .output()
    .expect("empty command should execute");
  assert!(
    empty.status.success(),
    "{}",
    String::from_utf8_lossy(&empty.stderr)
  );
  assert!(
    String::from_utf8(empty.stdout)
      .expect("empty stdout should be UTF-8")
      .contains("No matching context found.")
  );

  let invalid = agent_context_command(&fixture, "BENE_ID", "0", false)
    .output()
    .expect("invalid command should execute");
  assert_eq!(invalid.status.code(), Some(1));
  assert!(
    String::from_utf8(invalid.stderr)
      .expect("invalid stderr should be UTF-8")
      .contains("limit must be greater than 0")
  );
}
