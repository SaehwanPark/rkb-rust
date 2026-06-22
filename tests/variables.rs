#![allow(
  clippy::missing_errors_doc,
  clippy::must_use_candidate,
  clippy::too_many_lines,
  clippy::items_after_statements
)]

use rkb::cli::{Command, VariablesArgs};
use rkb::config::VariableExtractionConfig;
use rkb::records::ChunkMetadata;
use rkb::variables::{extract_variables_from_chunk, run_variable_extraction};
use std::fs;
use std::path::PathBuf;

fn test_dir(name: &str) -> PathBuf {
  PathBuf::from("_workspace/test_runs_mocked").join(format!(
    "{}_{}_{}",
    chrono::Utc::now().timestamp_nanos_opt().unwrap(),
    std::process::id(),
    name
  ))
}

fn chunk(source_document: String, text: &str) -> ChunkMetadata {
  ChunkMetadata {
    chunk_id: "chunk-1".to_string(),
    source_document,
    page: Some(3),
    text: text.to_string(),
    dataset: "mbsf".to_string(),
    url: "https://resdac.org/cms-data/files/mbsf/data-documentation".to_string(),
  }
}

#[test]
fn extracts_only_candidates_with_definition_evidence() {
  let input = chunk(
    "/tmp/source.txt".to_string(),
    "BENE_ID: Beneficiary identifier, also known as beneficiary id, 2020.\nCLM_ID appears elsewhere.",
  );

  let (rows, skipped) = extract_variables_from_chunk(&input);

  assert_eq!(skipped, 1);
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0].variable_id, "mbsf__var__bene-id");
  assert_eq!(rows[0].definition, "Beneficiary identifier, also known as beneficiary id, 2020");
  assert_eq!(rows[0].aliases.as_deref(), Some("beneficiary id"));
  assert_eq!(rows[0].years.as_deref(), Some("2020"));
  assert_eq!(rows[0].page, Some(3));
}

#[test]
fn writes_variable_and_canonical_artifacts_with_resolved_citations() {
  let root = test_dir("variable_pipeline");
  let parsed = root.join("parsed");
  let raw = root.join("raw");
  let metadata = root.join("metadata");
  let graph = root.join("graph");
  let workspace = root.join("workspace");
  fs::create_dir_all(&parsed).unwrap();
  fs::create_dir_all(&raw).unwrap();

  let source = parsed.join("mbsf.txt");
  fs::write(&source, "source").unwrap();
  let chunks = parsed.join("chunks.jsonl");
  let first = serde_json::to_string(&chunk(
    source.display().to_string(),
    "BENE_ID: Beneficiary identifier.",
  ))
  .unwrap();
  let mut second_chunk = chunk(
    source.display().to_string(),
    "BENE_ID: A longer beneficiary identifier definition.",
  );
  second_chunk.chunk_id = "chunk-2".to_string();
  let second = serde_json::to_string(&second_chunk).unwrap();
  fs::write(&chunks, format!("{first}\n{{not-json}}\n{second}\n")).unwrap();

  let variable_page = raw.join("encrypted-ccw-beneficiary-id.html");
  let html = r#"<html><head><title>Encrypted CCW Beneficiary ID | ResDAC</title></head>
  <body><h1>Encrypted CCW Beneficiary ID</h1><table>
  <tr><th>SAS Name</th><td>BENE_ID</td></tr>
  <tr><th>Definition</th><td>The unique CCW identifier for a beneficiary.</td></tr>
  </table><a href="/cms-data/files/mbsf/data-documentation">MBSF</a>
  <a href="/cms-data/files/mbsf/data-documentation">MBSF duplicate</a></body></html>"#;
  fs::write(&variable_page, html).unwrap();
  let manifest = root.join("archive_manifest.csv");
  fs::write(
    &manifest,
    format!(
      "url,resource_kind,asset_kind,source_url,source_title,content_type,http_status,archive_state,downloaded_at_utc,sha256,local_path,error\nhttps://resdac.org/cms-data/variables/encrypted-ccw-beneficiary-id,variable_page,,,,text/html,200,archived,2026-06-16T00:00:00Z,unused,{},\n",
      variable_page.display()
    ),
  )
  .unwrap();

  let config = VariableExtractionConfig {
    chunks_jsonl_path: chunks,
    archive_manifest_path: manifest,
    metadata_dir: metadata.clone(),
    graph_dir: graph.clone(),
    workspace_dir: workspace.clone(),
  };
  let result = run_variable_extraction(&config).unwrap();

  assert_eq!(result.chunks_read, 2);
  assert_eq!(result.variables.len(), 1);
  assert_eq!(result.failures.len(), 1);
  assert_eq!(result.failures[0].chunk_id, "line-2");
  assert_eq!(result.canonical_variables.len(), 1);
  assert_eq!(result.data_source_variable_edges.len(), 1);
  assert_eq!(
    result.variables[0].source_url,
    "https://resdac.org/cms-data/variables/encrypted-ccw-beneficiary-id"
  );
  assert_eq!(result.variables[0].source_document, variable_page.display().to_string());

  let variables_csv = fs::read_to_string(metadata.join("variables.csv")).unwrap();
  assert!(variables_csv.starts_with("variable_id,variable_name,dataset_id,definition,aliases,years,source_document,source_url,page,chunk_id,extraction_notes\n"));
  assert!(variables_csv.contains("chunk-2"));
  let canonical_csv = fs::read_to_string(metadata.join("canonical_variables.csv")).unwrap();
  assert!(canonical_csv.starts_with("variable_id,variable_name,variable_label,definition,source,source_url,source_document,extraction_notes\n"));
  let edges_csv = fs::read_to_string(graph.join("variable_edges.csv")).unwrap();
  assert!(edges_csv.starts_with("source_id,target_id,relationship,source_url,source_document,page,chunk_id\n"));
  let data_source_edges = fs::read_to_string(graph.join("data_source_variable_edges.csv")).unwrap();
  assert!(data_source_edges.starts_with("source_id,target_id,relationship,source_url,source_document,variable_url,variable_document,evidence_type,page,chunk_id\n"));
  let summary = fs::read_to_string(workspace.join("07_variable_pack.md")).unwrap();
  assert!(summary.contains("- Variables: 1"));
  assert!(summary.contains("- Failures: 1"));

  let _ = fs::remove_dir_all(root);
}

#[test]
fn command_reports_partial_failures_after_writing_outputs() {
  let root = test_dir("variable_failures");
  fs::create_dir_all(&root).unwrap();
  let chunks = root.join("chunks.jsonl");
  let missing = root.join("missing.txt");
  fs::write(
    &chunks,
    format!(
      "{}\n",
      serde_json::to_string(&chunk(missing.display().to_string(), "BENE_ID: Beneficiary identifier.")).unwrap()
    ),
  )
  .unwrap();

  let args = VariablesArgs {
    chunks_jsonl: chunks,
    archive_manifest: root.join("missing-manifest.csv"),
    metadata_dir: root.join("metadata"),
    graph_dir: root.join("graph"),
    workspace_dir: root.join("workspace"),
  };
  let result = rkb::run(Command::Variables(args));

  assert!(result.unwrap_err().to_string().contains("variable extraction completed with 1 failures"));
  assert!(root.join("metadata/variables.csv").is_file());
  assert!(root.join("workspace/07_variable_pack.md").is_file());

  let _ = fs::remove_dir_all(root);
}
