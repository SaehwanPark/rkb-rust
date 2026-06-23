use rkb::config::{RetrievalConfig, VariableEvaluationConfig};
use rkb::evaluation::{
  BenchmarkQuestion, BenchmarkQuestionSuite, citation_accuracy, evaluate_benchmark_suite,
  evaluate_variable_retrieval, generate_markdown_report, recall_at_k, reciprocal_rank,
};
use rkb::retrieval::build_index;
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

fn assert_close(actual: f64, expected: f64) {
  assert!(
    (actual - expected).abs() < f64::EPSILON,
    "expected {actual} to equal {expected}"
  );
}

fn fixture() -> Fixture {
  let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
  let root = std::env::temp_dir().join(format!("rkb-evaluation-{}-{sequence}", std::process::id()));
  let metadata = root.join("data/metadata");
  let parsed = root.join("data/parsed");

  let datasets = metadata.join("datasets.csv");
  write(
    &datasets,
    "dataset_id,name,program,category,availability,source_url,local_path,sha256,extraction_notes\nmedpar,MedPAR,Medicare,Claims,Available,https://resdac.org/cms-data/files/medpar,raw/medpar.html,fake-sha,\npde,PDE,Medicare,Part D,Available,https://resdac.org/cms-data/files/pde,raw/pde.html,fake-sha,\n",
  );
  let documents = metadata.join("documents.csv");
  write(
    &documents,
    "document_id,dataset_id,title,document_kind,source_url,local_path,sha256,content_type,extraction_notes\n",
  );
  let variables = metadata.join("variables.csv");
  write(
    &variables,
    "variable_id,variable_name,dataset_id,definition,aliases,years,source_document,source_url,page,chunk_id,extraction_notes\nmedpar__var__bene-id,BENE_ID,medpar,CCW Encrypted Beneficiary ID Number,,,raw/medpar-data.html,https://resdac.org/cms-data/files/medpar/data-documentation,,chunk-1,\npde__var__pde-id,PDE_ID,pde,Prescription drug event identifier,,,raw/pde-data.html,https://resdac.org/cms-data/files/pde/data-documentation,,chunk-2,\n",
  );
  let chunks = parsed.join("chunks.jsonl");
  write(
    &chunks,
    "{\"chunk_id\":\"chunk-1\",\"source_document\":\"raw/medpar-data.html\",\"page\":null,\"text\":\"| BENE_ID | CCW Encrypted Beneficiary ID Number |\",\"dataset\":\"medpar\",\"url\":\"https://resdac.org/cms-data/files/medpar/data-documentation\"}\n",
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

fn evaluate_command(fixture: &Fixture, json: bool) -> Command {
  let mut command = Command::new(env!("CARGO_BIN_EXE_rkb"));
  command.args([
    "evaluate",
    "--sample-size",
    "1",
    "--seed",
    "20260616",
    "--limit",
    "5",
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
fn evaluates_seeded_variable_sample_deterministically() {
  let fixture = fixture();
  let variables = fs::read_to_string(&fixture.config.variables_metadata_path)
    .expect("variables should be readable")
    .replace("BENE_ID,medpar", " BENE_ID ,medpar");
  fs::write(&fixture.config.variables_metadata_path, variables)
    .expect("variables should be replaceable");
  build_index(&fixture.config).expect("index should build");
  let config = VariableEvaluationConfig {
    retrieval: fixture.config.clone(),
    sample_size: 1,
    seed: 20_260_616,
    limit: 5,
    ..VariableEvaluationConfig::default()
  };

  let first = evaluate_variable_retrieval(&config).expect("evaluation should pass");
  let second = evaluate_variable_retrieval(&config).expect("evaluation should repeat");

  assert_eq!(
    first
      .cases
      .iter()
      .map(|case| case.variable_name.as_str())
      .collect::<Vec<_>>(),
    second
      .cases
      .iter()
      .map(|case| case.variable_name.as_str())
      .collect::<Vec<_>>()
  );
  assert_eq!(first.sample_size, 1);
  assert_eq!(first.passed_count(), 1);
  assert_close(first.pass_rate(), 1.0);
  assert!(first.cases[0].passed);
  assert_eq!(first.cases[0].first_matching_rank, Some(1));
  assert!(first.cases[0].citation_present);
}

#[test]
fn computes_evaluation_metrics() {
  assert_close(recall_at_k(&["a", "b", "c"], &["a", "d"], 2), 0.5);
  assert_close(recall_at_k(&["a", "b", "c"], &["a", "b"], 2), 1.0);
  assert_close(recall_at_k(&["a", "b", "c"], &[], 5), 1.0);

  assert_close(reciprocal_rank(&["a", "b", "c"], &["b", "d"]), 0.5);
  assert_close(reciprocal_rank(&["a", "b", "c"], &["a"]), 1.0);
  assert_close(reciprocal_rank(&["a", "b", "c"], &["d"]), 0.0);
  assert_close(reciprocal_rank(&["a", "b", "c"], &[]), 1.0);

  assert_close(
    citation_accuracy(
      &["http://a.com/".to_string(), "http://b.com".to_string()],
      &["http://a.com".to_string()],
    ),
    1.0,
  );
  assert_close(
    citation_accuracy(
      &["http://a.com".to_string()],
      &["http://a.com".to_string(), "http://b.com".to_string()],
    ),
    0.5,
  );
  assert_close(citation_accuracy(&[], &[]), 1.0);
}

#[test]
fn benchmark_suite_writes_markdown_report() {
  let fixture = fixture();
  build_index(&fixture.config).expect("index should build");
  let config = VariableEvaluationConfig {
    retrieval: fixture.config.clone(),
    sample_size: 1,
    seed: 20_260_616,
    limit: 5,
    ..VariableEvaluationConfig::default()
  };
  let suite = BenchmarkQuestionSuite {
    questions: vec![BenchmarkQuestion {
      question_id: "q1".to_string(),
      query: "BENE_ID".to_string(),
      expected_datasets: vec!["medpar".to_string()],
      expected_variables: vec!["BENE_ID".to_string()],
      expected_documents: Vec::new(),
      expected_citations: vec![
        "https://resdac.org/cms-data/files/medpar/data-documentation".to_string(),
      ],
      description: "Beneficiary identifier query".to_string(),
    }],
  };

  let report = evaluate_benchmark_suite(&config, &suite).expect("benchmark should run");
  assert_eq!(report.results.len(), 1);
  assert_eq!(report.results[0].question_id, "q1");
  assert_close(report.results[0].lexical.dataset_recall_at_5, 1.0);
  assert_eq!(report.results[0].hybrid, report.results[0].lexical);
  assert_close(report.results[0].agent_facing.variable_recall_at_5, 1.0);

  let output = fixture.root.join("report.md");
  generate_markdown_report(&report, &output).expect("report should be written");
  let markdown = fs::read_to_string(output).expect("report should be readable");
  assert!(markdown.contains("Aggregate Benchmark Summary"));
  assert!(markdown.contains("Query: `BENE_ID`"));
}

#[test]
fn evaluate_command_emits_text_and_json() {
  let fixture = fixture();
  build_index(&fixture.config).expect("index should build");

  let text = evaluate_command(&fixture, false)
    .output()
    .expect("evaluate text command should execute");
  assert!(
    text.status.success(),
    "{}",
    String::from_utf8_lossy(&text.stderr)
  );
  assert!(
    String::from_utf8(text.stdout)
      .expect("stdout should be UTF-8")
      .contains("variable retrieval usefulness: 1/1 passed")
  );

  let json = evaluate_command(&fixture, true)
    .output()
    .expect("evaluate JSON command should execute");
  assert!(
    json.status.success(),
    "{}",
    String::from_utf8_lossy(&json.stderr)
  );
  let payload: serde_json::Value =
    serde_json::from_slice(&json.stdout).expect("JSON output should parse");
  assert_eq!(payload["sample_size"], 1);
  assert_eq!(payload["passed_count"], 1);
  assert_eq!(payload["pass_rate"], 1.0);
  assert_eq!(payload["cases"][0]["first_matching_rank"], 1);
}

#[test]
fn evaluate_command_handles_benchmark_and_failures() {
  let fixture = fixture();
  build_index(&fixture.config).expect("index should build");
  let benchmark = fixture.root.join("benchmark_questions.json");
  write(
    &benchmark,
    r#"[{"question_id":"q1","query":"BENE_ID","expected_datasets":["medpar"],"expected_variables":["BENE_ID"],"expected_citations":["https://resdac.org/cms-data/files/medpar/data-documentation"],"description":"Beneficiary identifier query"}]"#,
  );
  let report = fixture.root.join("retrieval_evaluation_report.md");

  let output = Command::new(env!("CARGO_BIN_EXE_rkb"))
    .args([
      "evaluate",
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
      "--benchmark",
      path_str(&benchmark),
      "--output-report",
      path_str(&report),
    ])
    .output()
    .expect("benchmark command should execute");
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(report.is_file());
  assert!(
    String::from_utf8(output.stdout)
      .expect("stdout should be UTF-8")
      .contains("CMS Retrieval Benchmark Suite Evaluation")
  );

  let invalid = Command::new(env!("CARGO_BIN_EXE_rkb"))
    .args(["evaluate", "--sample-size", "0"])
    .output()
    .expect("invalid command should execute");
  assert_eq!(invalid.status.code(), Some(1));
  assert!(
    String::from_utf8(invalid.stderr)
      .expect("stderr should be UTF-8")
      .contains("sample_size must be greater than 0")
  );
}
