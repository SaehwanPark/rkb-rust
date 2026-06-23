use rkb::config::RetrievalConfig;
use rkb::retrieval::{build_index, build_index_with_options, run_retrieval};
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
    "rkb-hybrid-retrieval-{}-{sequence}",
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

fn add_paths(command: &mut Command, fixture: &Fixture) {
  command.args([
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
}

#[test]
fn builds_index_with_embedding_table() {
  let fixture = fixture();
  assert_eq!(build_index_with_options(&fixture.config, true).unwrap(), 4);

  let connection = rusqlite::Connection::open(&fixture.config.database_path).unwrap();
  let count: usize = connection
    .query_row("SELECT COUNT(*) FROM record_embeddings", [], |row| {
      row.get(0)
    })
    .unwrap();
  let bytes: Vec<u8> = connection
    .query_row(
      "SELECT embedding FROM record_embeddings LIMIT 1",
      [],
      |row| row.get(0),
    )
    .unwrap();
  assert_eq!(count, 4);
  assert_eq!(bytes.len(), 384 * std::mem::size_of::<f32>());
}

#[test]
fn hybrid_search_falls_back_without_embedding_table() {
  let fixture = fixture();
  build_index(&fixture.config).unwrap();
  let mut config = fixture.config.clone();
  config.hybrid_search_enabled = true;

  let results = run_retrieval(&config, "dual eligibility", 5).unwrap();
  assert_eq!(results[0].record_id, "chunk-1");
}

#[test]
fn exact_identifier_boost_wins_in_hybrid() {
  let fixture = fixture();
  build_index_with_options(&fixture.config, true).unwrap();
  let mut config = fixture.config.clone();
  config.hybrid_search_enabled = true;
  config.semantic_weight = 0.8;

  let results = run_retrieval(&config, "BENE_ID", 5).unwrap();
  assert_eq!(results[0].record_type.as_str(), "variable");
  assert_eq!(results[0].record_id, "mbsf__var__bene-id");
}

#[test]
fn hybrid_cli_flags_build_and_search() {
  let fixture = fixture();
  let mut index = Command::new(env!("CARGO_BIN_EXE_rkb"));
  index.args(["index", "--build-embeddings"]);
  add_paths(&mut index, &fixture);
  let output = index.output().expect("index should run");
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );

  let mut search = Command::new(env!("CARGO_BIN_EXE_rkb"));
  search.args([
    "search",
    "--query",
    "BENE_ID",
    "--limit",
    "1",
    "--hybrid",
    "--semantic-weight",
    "0.8",
    "--json",
  ]);
  add_paths(&mut search, &fixture);
  let output = search.output().expect("search should run");
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  assert_eq!(payload[0]["record_id"], "mbsf__var__bene-id");
}
