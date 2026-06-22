#![allow(
  clippy::missing_errors_doc,
  clippy::must_use_candidate,
  clippy::implicit_hasher,
  clippy::if_not_else,
  clippy::module_name_repetitions,
  clippy::too_many_lines,
  clippy::cast_possible_truncation,
  clippy::cast_sign_loss,
  clippy::cast_precision_loss,
  clippy::manual_string_new,
  clippy::items_after_statements,
  unused_imports
)]

use rkb::cli::{Command, ExtractArgs};
use rkb::error::AppError;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn compute_sha256(content: &[u8]) -> String {
  let mut hasher = Sha256::new();
  hasher.update(content);
  format!("{:x}", hasher.finalize())
}

#[test]
fn test_run_extraction_mocked() {
  // Set up mock directory
  let test_dir = PathBuf::from("_workspace/test_runs_mocked").join(format!(
    "{}_run_mocked",
    chrono::Utc::now().timestamp_millis()
  ));
  let raw_root = test_dir.join("raw");
  let metadata_dir = test_dir.join("metadata");
  let graph_dir = test_dir.join("graph");
  let workspace_dir = test_dir.join("workspace");
  fs::create_dir_all(&raw_root).unwrap();

  // 1. Create dataset HTML file
  let dataset_html = r#"
    <!DOCTYPE html>
    <html>
      <head><title>Accountable Health Communities Model | ResDAC</title></head>
      <body>
        <div class="views-field-field-program-type">
          <div class="field-content">Medicare, Medicaid</div>
        </div>
        <div class="views-field-field-data-file-category">
          <div class="field-content">Special Programs</div>
        </div>
        <div class="views-field-field-availability">
          <div class="field-content">May 2018-April 2023</div>
        </div>
        <a href="https://cms.gov/Regulations-and-Guidance/Guidance/Manuals/Downloads/bp102c07.pdf">Manual</a>
      </body>
    </html>
  "#;
  let dataset_path = raw_root.join("dataset.html");
  fs::write(&dataset_path, dataset_html).unwrap();
  let dataset_sha = compute_sha256(dataset_html.as_bytes());

  // 2. Create asset PDF file (dummy content)
  let asset_content = b"PDF dummy content";
  let asset_path = raw_root.join("bp102c07.pdf");
  fs::write(&asset_path, asset_content).unwrap();
  let asset_sha = compute_sha256(asset_content);

  // 3. Create archive_manifest.csv content
  let manifest_content = format!(
    "url,resource_kind,asset_kind,source_url,source_title,content_type,http_status,archive_state,downloaded_at_utc,sha256,local_path,error\n\
     https://resdac.org/cms-data/files/ahc-model,dataset_page,,https://resdac.org/cms-data,AHC Model,text/html,200,archived,2026-06-21T00:00:00Z,{},{},\n\
     https://cms.gov/Regulations-and-Guidance/Guidance/Manuals/Downloads/bp102c07.pdf,asset,pdf,https://resdac.org/cms-data/files/ahc-model,Manual,application/pdf,200,archived,2026-06-21T00:00:00Z,{},{},\n",
    dataset_sha,
    dataset_path.display(),
    asset_sha,
    asset_path.display()
  );
  let manifest_path = test_dir.join("archive_manifest.csv");
  fs::write(&manifest_path, manifest_content).unwrap();

  // Run extraction
  let args = ExtractArgs {
    archive_manifest: manifest_path,
    metadata_dir: metadata_dir.clone(),
    graph_dir: graph_dir.clone(),
    workspace_dir: workspace_dir.clone(),
  };

  let res = rkb::run(Command::Extract(args));
  assert!(res.is_ok(), "run_extraction failed: {res:?}");

  // Check datasets.csv output
  let datasets_csv = fs::read_to_string(metadata_dir.join("datasets.csv")).unwrap();
  assert!(datasets_csv.contains("ahc-model"));
  assert!(datasets_csv.contains("Medicare, Medicaid"));
  assert!(datasets_csv.contains("Special Programs"));
  assert!(datasets_csv.contains("May 2018-April 2023"));

  // Check documents.csv output
  let documents_csv = fs::read_to_string(metadata_dir.join("documents.csv")).unwrap();
  // Document ID should format as: dataset_id__suffix__stable_url_hash
  // ahc-model__bp102c07_pdf__551d41d9a2
  assert!(documents_csv.contains("ahc-model__bp102c07_pdf__551d41d9a2"));
  assert!(documents_csv.contains("Manual"));
  assert!(documents_csv.contains("pdf"));

  // Check document_edges.csv output
  let edges_csv = fs::read_to_string(graph_dir.join("document_edges.csv")).unwrap();
  assert!(edges_csv.contains("ahc-model,ahc-model__bp102c07_pdf__551d41d9a2,has_document"));

  // Check ontology nodes and edges
  let ontology_nodes = fs::read_to_string(graph_dir.join("ontology_nodes.csv")).unwrap();
  assert!(ontology_nodes.contains("ahc-model,Dataset,Accountable Health Communities Model"));
  assert!(ontology_nodes.contains("program_medicare-medicaid,Program,\"Medicare, Medicaid\""));

  let ontology_edges = fs::read_to_string(graph_dir.join("ontology_edges.csv")).unwrap();
  assert!(ontology_edges.contains("ahc-model,program_medicare-medicaid,belongs_to"));

  // Verify workspace extraction pack summary
  let summary = fs::read_to_string(workspace_dir.join("04_extraction_pack.md")).unwrap();
  assert!(summary.contains("- Datasets: 1"));
  assert!(summary.contains("- Documents: 1"));
  assert!(summary.contains("- Document edges: 1"));
  assert!(summary.contains("- Failures: 0"));

  // Cleanup test outputs
  let _ = fs::remove_dir_all(test_dir);
}

#[test]
fn test_extraction_validation_failure() {
  let test_dir = PathBuf::from("_workspace/test_runs_mocked").join(format!(
    "{}_val_failure",
    chrono::Utc::now().timestamp_millis()
  ));
  let raw_root = test_dir.join("raw");
  let metadata_dir = test_dir.join("metadata");
  let graph_dir = test_dir.join("graph");
  let workspace_dir = test_dir.join("workspace");
  fs::create_dir_all(&raw_root).unwrap();

  // Create invalid checksum manifest
  let manifest_content = "\
     url,resource_kind,asset_kind,source_url,source_title,content_type,http_status,archive_state,downloaded_at_utc,sha256,local_path,error\n\
     https://resdac.org/cms-data/files/ahc-model,dataset_page,,https://resdac.org/cms-data,AHC Model,text/html,200,archived,2026-06-21T00:00:00Z,wronghash,nonexistent_file.html,\n";
  let manifest_path = test_dir.join("archive_manifest.csv");
  fs::write(&manifest_path, manifest_content).unwrap();

  let args = ExtractArgs {
    archive_manifest: manifest_path,
    metadata_dir,
    graph_dir,
    workspace_dir: workspace_dir.clone(),
  };

  let res = rkb::run(Command::Extract(args));
  assert!(res.is_err());
  assert!(
    res
      .unwrap_err()
      .to_string()
      .contains("extraction completed with 1 failures")
  );

  let summary = fs::read_to_string(workspace_dir.join("04_extraction_pack.md")).unwrap();
  assert!(summary.contains("- Failures: 1"));
  assert!(summary.contains("nonexistent_file.html"));
  assert!(summary.contains("archived file does not exist"));

  let _ = fs::remove_dir_all(test_dir);
}
