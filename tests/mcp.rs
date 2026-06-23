use rkb::config::RetrievalConfig;
use rkb::mcp::{
  McpConfig, call_tool, read_background_state, run_stdio_server, start_background_state,
  stop_background_state, tool_names,
};
use rkb::retrieval::build_index;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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
  let root = std::env::temp_dir().join(format!("rkb-mcp-{}-{sequence}", std::process::id()));
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

fn mcp_config(fixture: &Fixture) -> McpConfig {
  McpConfig {
    retrieval: fixture.config.clone(),
    default_limit: 5,
  }
}

#[test]
fn registers_expected_mcp_tools() {
  assert_eq!(
    tool_names(),
    vec![
      "search_datasets",
      "search_documents",
      "search_variables",
      "search_chunks",
      "get_agent_context"
    ]
  );
}

#[test]
fn mcp_tools_search_by_record_type_and_context() {
  let fixture = fixture();
  build_index(&fixture.config).expect("index should build");
  let config = mcp_config(&fixture);

  let datasets: serde_json::Value =
    serde_json::from_str(&call_tool(&config, "search_datasets", "mbsf", None).unwrap())
      .expect("dataset results should parse");
  assert_eq!(datasets[0]["record_id"], "mbsf");
  assert_eq!(datasets[0]["record_type"], "dataset");

  let variables: serde_json::Value =
    serde_json::from_str(&call_tool(&config, "search_variables", "BENE_ID", Some(1)).unwrap())
      .expect("variable results should parse");
  assert_eq!(variables[0]["record_id"], "mbsf__var__bene-id");
  assert_eq!(variables[0]["record_type"], "variable");

  let context: serde_json::Value =
    serde_json::from_str(&call_tool(&config, "get_agent_context", "BENE_ID", Some(1)).unwrap())
      .expect("context should parse");
  assert_eq!(context["query"], "BENE_ID");
  assert_eq!(context["entries"][0]["record_id"], "mbsf__var__bene-id");
  assert_eq!(context["entries"][0]["citation"], "[1]");
}

#[test]
fn mcp_tools_reject_invalid_queries_and_unknown_tools() {
  let fixture = fixture();
  build_index(&fixture.config).expect("index should build");
  let config = mcp_config(&fixture);

  assert!(
    call_tool(&config, "search_variables", " ", None)
      .expect_err("blank query should fail")
      .to_string()
      .contains("query must not be empty")
  );
  assert!(
    call_tool(&config, "unknown", "BENE_ID", None)
      .expect_err("unknown tool should fail")
      .to_string()
      .contains("unknown MCP tool")
  );
}

#[test]
fn stdio_server_handles_list_and_tool_call_requests() {
  let fixture = fixture();
  build_index(&fixture.config).expect("index should build");
  let config = mcp_config(&fixture);
  let input = concat!(
    "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n",
    "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"search_variables\",\"arguments\":{\"query\":\"BENE_ID\",\"limit\":1}}}\n"
  );
  let mut output = Vec::new();

  run_stdio_server(&config, Cursor::new(input), &mut output).expect("stdio server should run");

  let lines = String::from_utf8(output).expect("response should be UTF-8");
  let responses = lines
    .lines()
    .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("line should parse"))
    .collect::<Vec<_>>();
  assert_eq!(
    responses[0]["result"]["tools"][0]["name"],
    "search_datasets"
  );
  let content = responses[1]["result"]["content"][0]["text"]
    .as_str()
    .expect("tool result text should be a string");
  let payload: serde_json::Value = serde_json::from_str(content).expect("payload should parse");
  assert_eq!(payload[0]["record_id"], "mbsf__var__bene-id");
}

#[test]
fn mcp_command_accepts_json_rpc_on_stdin() {
  let fixture = fixture();
  build_index(&fixture.config).expect("index should build");
  let mut child = Command::new(env!("CARGO_BIN_EXE_rkb"))
    .args([
      "mcp",
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
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("mcp command should spawn");
  {
    use std::io::Write;
    let stdin = child.stdin.as_mut().expect("stdin should be open");
    stdin
      .write_all(
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"search_chunks\",\"arguments\":{\"query\":\"Dual eligibility\",\"limit\":1}}}\n",
      )
      .expect("request should write");
  }
  drop(child.stdin.take());
  let output = child.wait_with_output().expect("mcp command should finish");

  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  let response: serde_json::Value =
    serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
  let text = response["result"]["content"][0]["text"]
    .as_str()
    .expect("tool text should be a string");
  let payload: serde_json::Value = serde_json::from_str(text).expect("payload should parse");
  assert_eq!(payload[0]["record_id"], "chunk-1");
}

#[test]
fn mcp_lifecycle_records_status_and_stop_state() {
  let fixture = fixture();
  let workspace = fixture.root.join("_workspace");

  assert!(read_background_state(&workspace).unwrap().is_none());
  let state = start_background_state(&workspace, "127.0.0.1", 9000).unwrap();
  assert_eq!(state.host, "127.0.0.1");
  assert_eq!(state.port, 9000);
  assert_eq!(state.endpoint_url, "http://127.0.0.1:9000/sse");
  assert_eq!(
    read_background_state(&workspace).unwrap().unwrap().port,
    9000
  );
  assert_eq!(stop_background_state(&workspace).unwrap().port, 9000);
  assert!(read_background_state(&workspace).unwrap().is_none());
}

#[test]
fn mcp_lifecycle_cli_reports_state_transitions() {
  let fixture = fixture();
  let workspace = fixture.root.join("_workspace");
  let workspace_arg = path_str(&workspace);

  let stopped = Command::new(env!("CARGO_BIN_EXE_rkb"))
    .args(["mcp", "--workspace-dir", workspace_arg, "status"])
    .output()
    .expect("status command should execute");
  assert!(stopped.status.success());
  assert!(
    String::from_utf8(stopped.stdout)
      .unwrap()
      .contains("MCP server status: stopped")
  );

  let start = Command::new(env!("CARGO_BIN_EXE_rkb"))
    .args([
      "mcp",
      "--workspace-dir",
      workspace_arg,
      "start",
      "--host",
      "127.0.0.1",
      "--port",
      "9000",
    ])
    .output()
    .expect("start command should execute");
  assert!(
    start.status.success(),
    "{}",
    String::from_utf8_lossy(&start.stderr)
  );
  assert!(
    String::from_utf8(start.stdout)
      .unwrap()
      .contains("MCP server state recorded successfully")
  );

  let status = Command::new(env!("CARGO_BIN_EXE_rkb"))
    .args(["mcp", "--workspace-dir", workspace_arg, "status"])
    .output()
    .expect("status command should execute");
  assert!(status.status.success());
  let status_stdout = String::from_utf8(status.stdout).unwrap();
  assert!(status_stdout.contains("MCP server status: recorded"));
  assert!(status_stdout.contains("Port: 9000"));

  let stop = Command::new(env!("CARGO_BIN_EXE_rkb"))
    .args(["mcp", "--workspace-dir", workspace_arg, "stop"])
    .output()
    .expect("stop command should execute");
  assert!(
    stop.status.success(),
    "{}",
    String::from_utf8_lossy(&stop.stderr)
  );
  assert!(
    String::from_utf8(stop.stdout)
      .unwrap()
      .contains("MCP server stopped successfully.")
  );
}
