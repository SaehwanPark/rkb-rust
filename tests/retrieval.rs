use rkb::config::RetrievalConfig;
use rkb::retrieval::{build_index, load_retrievable_records, run_retrieval};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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

fn fixture() -> Fixture {
  let nonce = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("clock should follow epoch")
    .as_nanos();
  let root = std::env::temp_dir().join(format!("rkb-retrieval-{}-{nonce}", std::process::id()));
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

#[test]
fn loads_required_and_optional_records_with_citations() {
  let fixture = fixture();
  let records = load_retrievable_records(&fixture.config).expect("records should load");

  assert_eq!(
    records.iter().map(|record| record.record_type.as_str()).collect::<Vec<_>>(),
    ["dataset", "document", "variable", "chunk"]
  );
  assert!(records.iter().all(|record| !record.source_url.is_empty()));
  assert_eq!(records[2].page, Some(3));
}

#[test]
fn allows_missing_optional_inputs() {
  let fixture = fixture();
  fs::remove_file(&fixture.config.variables_metadata_path).expect("variables should be removed");
  fs::remove_file(&fixture.config.chunks_jsonl_path).expect("chunks should be removed");

  let records = load_retrievable_records(&fixture.config).expect("required records should load");
  assert_eq!(
    records.iter().map(|record| record.record_type.as_str()).collect::<Vec<_>>(),
    ["dataset", "document"]
  );
}

#[test]
fn rejects_blank_citations_and_malformed_chunks() {
  let fixture = fixture();
  let datasets = fs::read_to_string(&fixture.config.datasets_metadata_path)
    .expect("datasets should be readable")
    .replace("https://resdac.org/cms-data/files/mbsf", "");
  fs::write(&fixture.config.datasets_metadata_path, datasets).expect("datasets should be replaced");
  let error = load_retrievable_records(&fixture.config).expect_err("blank citation should fail");
  assert!(error.to_string().contains("empty required field: source_url"));

  let malformed_fixture = fixture();
  fs::write(&malformed_fixture.config.chunks_jsonl_path, "not-json\n")
    .expect("chunks should be replaced");
  let error = load_retrievable_records(&malformed_fixture.config)
    .expect_err("malformed chunk should fail");
  assert!(error.to_string().contains("failed to parse chunk JSON on line 1"));
}

#[test]
fn builds_and_rebuilds_fts_index() {
  let fixture = fixture();
  assert_eq!(build_index(&fixture.config).expect("index should build"), 4);
  assert!(fixture.config.database_path.is_file());
  assert!(!fixture.config.database_path.with_extension("sqlite.tmp").exists());
  assert_eq!(build_index(&fixture.config).expect("index should rebuild"), 4);

  let connection = rusqlite::Connection::open(&fixture.config.database_path)
    .expect("index should be readable");
  let records: usize = connection
    .query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))
    .expect("records should be counted");
  let fts_records: usize = connection
    .query_row("SELECT COUNT(*) FROM records_fts", [], |row| row.get(0))
    .expect("FTS records should be counted");
  assert_eq!((records, fts_records), (4, 4));
}

#[test]
fn ranks_exact_identifier_and_text_with_citations() {
  let fixture = fixture();
  build_index(&fixture.config).expect("index should build");

  let exact = run_retrieval(&fixture.config, "BENE_ID", 5).expect("identifier search should work");
  assert_eq!(exact[0].record_id, "mbsf__var__bene-id");
  assert_eq!(exact[0].record_type.as_str(), "variable");
  assert_eq!(exact[0].page, Some(3));

  let text = run_retrieval(&fixture.config, "dual eligibility", 5).expect("text search should work");
  assert_eq!(text[0].record_id, "chunk-1");
  assert!(text[0].snippet.contains("Dual eligibility"));
  assert_eq!(text[0].source_url, "https://resdac.org/cms-data/files/mbsf-codebook");
}

#[test]
fn validates_queries_and_returns_no_unmatched_results() {
  let fixture = fixture();
  build_index(&fixture.config).expect("index should build");

  for (query, limit, message) in [
    ("   ", 5, "query must not be empty"),
    ("!!!", 5, "query must contain at least one searchable token"),
    ("BENE_ID", 0, "limit must be greater than 0"),
  ] {
    let error = run_retrieval(&fixture.config, query, limit).expect_err("invalid query should fail");
    assert!(error.to_string().contains(message));
  }
  assert!(
    run_retrieval(&fixture.config, "unfindableterm", 5)
      .expect("unmatched query should work")
      .is_empty()
  );
}
