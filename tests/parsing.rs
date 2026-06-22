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

use rkb::cli::{Command, ParseArgs};
use rkb::error::AppError;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn test_run_parsing_pipeline_mocked() {
  let test_dir = PathBuf::from("_workspace/test_runs_mocked").join(format!(
    "{}_run_parsing_mocked",
    chrono::Utc::now().timestamp_millis()
  ));
  let metadata_dir = test_dir.join("metadata");
  let parsed_root = test_dir.join("parsed");
  let workspace_dir = test_dir.join("workspace");
  let raw_root = test_dir.join("raw");

  fs::create_dir_all(&metadata_dir).unwrap();
  fs::create_dir_all(&raw_root).unwrap();

  // 1. Write mock HTML dataset file
  let html_content = r#"
    <!DOCTYPE html>
    <html>
      <head><title>AHC Model | ResDAC</title></head>
      <body>
        <h1>AHC Model</h1>
        <div class="views-field-field-program-type">
          <div class="field-content">Medicare, Medicaid</div>
        </div>
        <div class="views-field-field-data-file-category">
          <div class="field-content">Special Programs</div>
        </div>
        <div class="views-field-field-availability">
          <div class="field-content">May 2018-April 2023</div>
        </div>
        <p>In 2017, the Innovation Center launched the Accountable Health Communities (AHC) Model to assess whether identifying and addressing Medicare and Medicaid beneficiaries' health-related social needs (HRSNs) would reduce health care use and costs.</p>
      </body>
    </html>
  "#;
  let dataset_path = raw_root.join("ahc-model.html");
  fs::write(&dataset_path, html_content).unwrap();

  // 2. Write datasets.csv
  let datasets_content = format!(
    "dataset_id,name,program,category,availability,source_url,local_path,sha256,extraction_notes\n\
     ahc-model,AHC Model,\"Medicare, Medicaid\",Special Programs,May 2018-April 2023,https://resdac.org/cms-data/files/ahc-model,{},fake_sha,\n",
    dataset_path.display()
  );
  let datasets_csv_path = metadata_dir.join("datasets.csv");
  fs::write(&datasets_csv_path, datasets_content).unwrap();

  // 3. Write documents.csv (empty or with a dummy HTML document for simplicity)
  let documents_content = "document_id,dataset_id,title,document_kind,source_url,local_path,sha256,content_type,extraction_notes\n";
  let documents_csv_path = metadata_dir.join("documents.csv");
  fs::write(&documents_csv_path, documents_content).unwrap();

  // 4. Run parse command
  let args = ParseArgs {
    datasets_metadata: datasets_csv_path,
    documents_metadata: documents_csv_path,
    parsed_root: parsed_root.clone(),
    workspace_dir: workspace_dir.clone(),
    chunk_size: 100,
    chunk_overlap: 20,
  };

  let res = rkb::run(Command::Parse(args));
  assert!(res.is_ok(), "run_parsing failed: {res:?}");

  // 5. Verify outputs
  let clean_text_path = parsed_root.join("html/ahc-model.txt");
  assert!(clean_text_path.exists());
  let clean_text = fs::read_to_string(clean_text_path).unwrap();
  assert!(clean_text.contains("AHC Model"));
  assert!(clean_text.contains("Medicare, Medicaid"));

  let chunks_jsonl_path = parsed_root.join("chunks.jsonl");
  assert!(chunks_jsonl_path.exists());
  let chunks_jsonl = fs::read_to_string(chunks_jsonl_path).unwrap();
  assert!(chunks_jsonl.contains("ahc-model__chunk_0"));

  let summary_path = workspace_dir.join("05_parsing_pack.md");
  assert!(summary_path.exists());
  let summary = fs::read_to_string(summary_path).unwrap();
  assert!(summary.contains("- Datasets parsed: 1"));

  // Cleanup
  let _ = fs::remove_dir_all(test_dir);
}

#[test]
fn test_run_parsing_failures() {
  let test_dir = PathBuf::from("_workspace/test_runs_mocked").join(format!(
    "{}_parsing_failures",
    chrono::Utc::now().timestamp_millis()
  ));
  let metadata_dir = test_dir.join("metadata");
  let parsed_root = test_dir.join("parsed");
  let workspace_dir = test_dir.join("workspace");

  fs::create_dir_all(&metadata_dir).unwrap();

  // 1. Write datasets.csv referencing a nonexistent local file
  let datasets_content = "\
     dataset_id,name,program,category,availability,source_url,local_path,sha256,extraction_notes\n\
     ahc-model,AHC Model,\"Medicare, Medicaid\",Special Programs,May 2018-April 2023,https://resdac.org/cms-data/files/ahc-model,nonexistent.html,fake_sha,\n";
  let datasets_csv_path = metadata_dir.join("datasets.csv");
  fs::write(&datasets_csv_path, datasets_content).unwrap();

  let documents_content = "document_id,dataset_id,title,document_kind,source_url,local_path,sha256,content_type,extraction_notes\n";
  let documents_csv_path = metadata_dir.join("documents.csv");
  fs::write(&documents_csv_path, documents_content).unwrap();

  let args = ParseArgs {
    datasets_metadata: datasets_csv_path,
    documents_metadata: documents_csv_path,
    parsed_root,
    workspace_dir: workspace_dir.clone(),
    chunk_size: 100,
    chunk_overlap: 20,
  };

  let res = rkb::run(Command::Parse(args));
  assert!(res.is_err());
  assert!(
    res
      .unwrap_err()
      .to_string()
      .contains("parsing completed with 1 failures")
  );

  let summary_path = workspace_dir.join("05_parsing_pack.md");
  assert!(summary_path.exists());
  let summary = fs::read_to_string(summary_path).unwrap();
  assert!(summary.contains("dataset file does not exist locally"));

  let _ = fs::remove_dir_all(test_dir);
}
