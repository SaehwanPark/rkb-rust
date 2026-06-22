use rkb::cli::{Command, QaArgs};
use rkb::config::QAConfig;
use rkb::qa::{QaVerdict, run_qa};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn test_dir(name: &str) -> PathBuf {
  PathBuf::from("_workspace/test_runs_mocked").join(format!(
    "{}_{}_{}",
    chrono::Utc::now().timestamp_nanos_opt().unwrap(),
    std::process::id(),
    name
  ))
}

fn write(path: &Path, bytes: &[u8]) -> String {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).unwrap();
  }
  fs::write(path, bytes).unwrap();
  format!("{:x}", Sha256::digest(bytes))
}

fn valid_fixture(name: &str) -> (PathBuf, QAConfig) {
  let root = test_dir(name);
  let raw = root.join("raw");
  let metadata = root.join("metadata");
  let graph = root.join("graph");
  let dataset_file = raw.join("dataset.html");
  let document_file = raw.join("document.pdf");
  let dataset_sha = write(&dataset_file, b"dataset");
  let document_sha = write(&document_file, b"document");
  let dataset_url = "https://resdac.org/cms-data/files/ds-1";
  let document_url = "https://resdac.org/cms-data/files/ds-1/doc-1";

  let datasets = metadata.join("datasets.csv");
  write(&datasets, format!("dataset_id,name,program,category,availability,source_url,local_path,sha256,extraction_notes\nds-1,Dataset,Medicare,Claims,Available,{dataset_url},{},{dataset_sha},\n", dataset_file.display()).as_bytes());
  let documents = metadata.join("documents.csv");
  write(&documents, format!("document_id,dataset_id,title,document_kind,source_url,local_path,sha256,content_type,extraction_notes\ndoc-1,ds-1,Document,pdf,{document_url},{},{document_sha},application/pdf,\n", document_file.display()).as_bytes());
  let edges = graph.join("document_edges.csv");
  write(&edges, format!("source_id,target_id,relationship,source_url,local_path,sha256\nds-1,doc-1,has_document,{document_url},{},{document_sha}\n", document_file.display()).as_bytes());
  let manifest = root.join("archive_manifest.csv");
  write(&manifest, format!("url,resource_kind,asset_kind,source_url,source_title,content_type,http_status,archive_state,downloaded_at_utc,sha256,local_path,error\n{dataset_url},dataset_page,,,,text/html,200,archived,2026-06-22T00:00:00Z,{dataset_sha},{},\n{document_url},documentation_page,,,,application/pdf,200,archived,2026-06-22T00:00:00Z,{document_sha},{},\n", dataset_file.display(), document_file.display()).as_bytes());

  let config = QAConfig {
    datasets_metadata_path: datasets,
    documents_metadata_path: documents,
    document_edges_path: edges,
    archive_manifest_path: manifest,
    workspace_dir: root.join("workspace"),
    variables_metadata_path: metadata.join("variables.csv"),
    canonical_variables_metadata_path: metadata.join("canonical_variables.csv"),
    variable_edges_path: graph.join("variable_edges.csv"),
    data_source_variable_edges_path: graph.join("data_source_variable_edges.csv"),
    ontology_nodes_path: graph.join("ontology_nodes.csv"),
    ontology_edges_path: graph.join("ontology_edges.csv"),
  };
  (root, config)
}

#[test]
fn valid_provenance_passes_and_writes_report() {
  let (root, config) = valid_fixture("qa_pass");
  let result = run_qa(&config).unwrap();
  assert_eq!(result.verdict, QaVerdict::Pass);
  assert!(result.findings.is_empty());
  assert_eq!(
    (
      result.datasets_checked,
      result.documents_checked,
      result.edges_checked
    ),
    (1, 1, 1)
  );
  assert!(
    fs::read_to_string(result.summary_path)
      .unwrap()
      .contains("Verdict: **PASS**")
  );
  fs::remove_dir_all(root).unwrap();
}

#[test]
fn bounded_integrity_failures_require_fix() {
  let (root, config) = valid_fixture("qa_fix");
  let mut datasets = fs::read_to_string(&config.datasets_metadata_path).unwrap();
  datasets = datasets.replace(
    &fs::read_to_string(&config.archive_manifest_path)
      .unwrap()
      .lines()
      .nth(1)
      .unwrap()
      .split(',')
      .nth(9)
      .unwrap()
      .to_string(),
    "wrong-sha",
  );
  fs::write(&config.datasets_metadata_path, datasets).unwrap();
  let result = run_qa(&config).unwrap();
  assert_eq!(result.verdict, QaVerdict::Fix);
  assert!(
    result
      .findings
      .iter()
      .any(|finding| finding.field == "sha256")
  );
  fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_archive_state_is_reported() {
  let (root, config) = valid_fixture("qa_archive_state");
  let manifest = fs::read_to_string(&config.archive_manifest_path)
    .unwrap()
    .replacen(",archived,", ",failed,", 1);
  fs::write(&config.archive_manifest_path, manifest).unwrap();
  let result = run_qa(&config).unwrap();
  assert_eq!(result.verdict, QaVerdict::Fix);
  assert!(result.findings.iter().any(|finding| {
    finding.field == "source_url" && finding.message.contains("archive state is 'failed'")
  }));
  fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_required_files_require_redo() {
  let root = test_dir("qa_redo");
  let config = QAConfig {
    workspace_dir: root.join("workspace"),
    ..QAConfig::default()
  };
  let result = run_qa(&config).unwrap();
  assert_eq!(result.verdict, QaVerdict::Redo);
  assert_eq!(result.error_count(), 3);
  assert!(result.summary_path.is_file());
  fs::remove_dir_all(root).unwrap();
}

#[test]
fn qa_command_returns_error_for_non_pass_verdict() {
  let root = test_dir("qa_command");
  let args = QaArgs::from_config(QAConfig {
    workspace_dir: root.join("workspace"),
    ..QAConfig::default()
  });
  let error = rkb::run(Command::Qa(args)).unwrap_err();
  assert!(
    error
      .to_string()
      .contains("QA review finished with verdict: REDO")
  );
  fs::remove_dir_all(root).unwrap();
}
