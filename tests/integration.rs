use rkb::config::RetrievalConfig;
use rkb::integration::{
  cohort_dictionary, crosswalk_variables, dataset_availability, format_context,
  parse_availability_years, scan_codebase_caveats,
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

fn fixture() -> Fixture {
  let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
  let root =
    std::env::temp_dir().join(format!("rkb-integration-{}-{sequence}", std::process::id()));
  let metadata = root.join("data/metadata");
  let parsed = root.join("data/parsed");

  let datasets = metadata.join("datasets.csv");
  write(
    &datasets,
    "dataset_id,name,program,category,availability,source_url,local_path,sha256,extraction_notes\ncarrier-ffs,Original Medicare (Fee-for-Service) Carrier,Medicare,Claims,Annual: 1999-2024 Monthly: 2025 and 2026,https://resdac.org/cms-data/files/carrier-ffs,raw/carrier.html,fake-sha,\nmbsf-base,Medicare Beneficiary Summary File Base,Medicare,Enrollment,May 2018-April 2023,https://resdac.org/cms-data/files/mbsf-base,raw/mbsf.html,fake-sha,\n",
  );
  let documents = metadata.join("documents.csv");
  write(
    &documents,
    "document_id,dataset_id,title,document_kind,source_url,local_path,sha256,content_type,extraction_notes\ncarrier__codebook,carrier-ffs,Carrier Codebook,pdf,https://resdac.org/cms-data/files/carrier-codebook,raw/carrier.pdf,fake-doc-sha,application/pdf,\n",
  );
  let variables = metadata.join("variables.csv");
  write(
    &variables,
    "variable_id,variable_name,dataset_id,definition,aliases,years,source_document,source_url,page,chunk_id,extraction_notes\ncarrier__var__bene-id,BENE_ID,carrier-ffs,Beneficiary identifier used to link claims and enrollment records.,beneficiary id,2020,parsed/carrier.txt,https://resdac.org/cms-data/files/carrier-codebook,3,chunk-1,\ncarrier__var__gndr-cd,GNDR_CD,carrier-ffs,Gender code for beneficiary.,gender,2020,parsed/carrier.txt,https://resdac.org/cms-data/files/carrier-codebook,4,chunk-2,\n",
  );
  let chunks = parsed.join("chunks.jsonl");
  write(
    &chunks,
    "{\"chunk_id\":\"chunk-1\",\"source_document\":\"parsed/carrier.txt\",\"page\":3,\"text\":\"BENE_ID identifies Medicare beneficiaries in Carrier claims.\",\"dataset\":\"carrier-ffs\",\"url\":\"https://resdac.org/cms-data/files/carrier-codebook\"}\n",
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

fn integration_command() -> Command {
  let mut command = Command::new(env!("CARGO_BIN_EXE_rkb"));
  command.arg("integration");
  command
}

#[test]
fn parses_availability_year_ranges() {
  assert_eq!(
    parse_availability_years("May 2018-April 2023"),
    vec![2018, 2019, 2020, 2021, 2022, 2023]
  );
  assert_eq!(
    parse_availability_years("2015-2019 2021-2025"),
    vec![2015, 2016, 2017, 2018, 2019, 2021, 2022, 2023, 2024, 2025]
  );
  assert_eq!(parse_availability_years("no years here"), Vec::<u16>::new());
}

#[test]
fn returns_availability_crosswalk_and_cohort_dictionary() {
  let fixture = fixture();
  let availability = dataset_availability(&fixture.config, "carrier-ffs").unwrap();
  assert_eq!(
    availability.name,
    "Original Medicare (Fee-for-Service) Carrier"
  );
  assert!(availability.available_years.contains(&1999));
  assert!(availability.available_years.contains(&2026));

  let crosswalk = crosswalk_variables(
    &fixture.config,
    &["bene_id".to_string(), "BENE_ID".to_string()],
  )
  .unwrap();
  assert!(
    crosswalk.variables["bene_id"][0]
      .available_years
      .contains(&2020)
  );
  assert_eq!(crosswalk.variables["BENE_ID"][0].variable_name, "BENE_ID");

  let cohort = cohort_dictionary(
    &fixture.config,
    &["BENE_ID".to_string(), "MISSING".to_string()],
  )
  .unwrap();
  assert_eq!(cohort["BENE_ID"][0].record_id, "carrier__var__bene-id");
  assert!(cohort["MISSING"].is_empty());
}

#[test]
fn formats_context_for_prompt_markdown_and_xml() {
  let context = rkb::agent_context::build_agent_context(
    "BENE_ID & GNDR_CD",
    vec![rkb::retrieval::SearchResult {
      record_id: "carrier__var__bene-id".to_string(),
      record_type: rkb::retrieval::RecordType::Variable,
      title: "BENE_ID \"Identifier\"".to_string(),
      dataset_id: "carrier-ffs".to_string(),
      score: 1.5,
      snippet: "Snippet containing <special> characters & symbols.".to_string(),
      source_url: "https://resdac.org/cms-data/variables/bene-id?p=1&q=2".to_string(),
      source_document: "parsed/carrier.txt".to_string(),
      page: None,
    }],
  );

  assert!(
    format_context(&context, "prompt")
      .unwrap()
      .contains("=== CMS DOCUMENTATION CONTEXT ===")
  );
  assert!(
    format_context(&context, "markdown")
      .unwrap()
      .contains("### CMS Documentation Context")
  );
  let xml = format_context(&context, "xml").unwrap();
  assert!(xml.contains("<documentation_context>"));
  assert!(xml.contains("BENE_ID &amp; GNDR_CD"));
  assert!(xml.contains("BENE_ID &quot;Identifier&quot;"));
  assert!(xml.contains("Snippet containing &lt;special&gt; characters &amp; symbols."));
}

#[test]
fn scans_codebase_caveats() {
  let fixture = fixture();
  let script = fixture.root.join("src/analysis.sas");
  write(
    &script,
    "data cohort; set lib.carrier; run; * check BENE_ID and mbsf-base here;",
  );

  let response =
    scan_codebase_caveats(&fixture.config, &[script], &["encounter".to_string()]).unwrap();
  assert!(response.matches.contains_key("BENE_ID"));
  assert!(response.matches.contains_key("mbsf-base"));
}

#[test]
fn integration_cli_emits_expected_outputs() {
  let fixture = fixture();
  build_index(&fixture.config).expect("index should build");

  let mut availability_command = integration_command();
  availability_command.args(["availability", "--dataset", "carrier-ffs"]);
  add_paths(&mut availability_command, &fixture);
  let availability = availability_command
    .output()
    .expect("availability should execute");
  assert!(
    availability.status.success(),
    "{}",
    String::from_utf8_lossy(&availability.stderr)
  );
  let payload: serde_json::Value =
    serde_json::from_slice(&availability.stdout).expect("availability should be JSON");
  assert_eq!(payload["dataset_id"], "carrier-ffs");

  let mut crosswalk_command = integration_command();
  crosswalk_command.args(["crosswalk", "--variables", "BENE_ID,bene_id"]);
  add_paths(&mut crosswalk_command, &fixture);
  let crosswalk = crosswalk_command
    .output()
    .expect("crosswalk should execute");
  assert!(
    crosswalk.status.success(),
    "{}",
    String::from_utf8_lossy(&crosswalk.stderr)
  );
  let payload: serde_json::Value =
    serde_json::from_slice(&crosswalk.stdout).expect("crosswalk should be JSON");
  assert!(payload["variables"]["BENE_ID"].is_array());
  assert!(payload["variables"]["bene_id"].is_array());

  let mut context_command = integration_command();
  context_command.args([
    "format-context",
    "--query",
    "BENE_ID",
    "--format",
    "markdown",
    "--limit",
    "1",
  ]);
  add_paths(&mut context_command, &fixture);
  let context = context_command
    .output()
    .expect("format context should execute");
  assert!(
    context.status.success(),
    "{}",
    String::from_utf8_lossy(&context.stderr)
  );
  assert!(
    String::from_utf8(context.stdout)
      .unwrap()
      .contains("### CMS Documentation Context")
  );
}
