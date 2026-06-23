//! Minimal stdio MCP-compatible tool surface over local retrieval.

use crate::agent_context;
use crate::config::RetrievalConfig;
use crate::error::AppError;
use crate::retrieval::{self, RecordType, SearchResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

const TOOLS: &[&str] = &[
  "search_datasets",
  "search_documents",
  "search_variables",
  "search_chunks",
  "get_agent_context",
];

#[derive(Clone, Debug, PartialEq)]
pub struct McpConfig {
  pub retrieval: RetrievalConfig,
  pub default_limit: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct McpServerState {
  pub pid: u32,
  pub host: String,
  pub port: u16,
  pub transport: String,
  pub endpoint_url: String,
  pub started_at_utc: String,
  pub log_path: PathBuf,
}

fn mcp_error(message: impl Into<String>) -> AppError {
  AppError::RetrievalError(message.into())
}

fn state_path(workspace_dir: &Path) -> PathBuf {
  workspace_dir.join("mcp_server_state.json")
}

fn log_path(workspace_dir: &Path) -> PathBuf {
  workspace_dir.join("mcp_server.log")
}

/// Records background MCP server startup metadata.
///
/// # Errors
///
/// Returns [`AppError`] when the state file cannot be written.
pub fn start_background_state(
  workspace_dir: &Path,
  host: &str,
  port: u16,
) -> Result<McpServerState, AppError> {
  fs::create_dir_all(workspace_dir).map_err(|error| {
    mcp_error(format!(
      "failed to create MCP workspace {}: {error}",
      workspace_dir.display()
    ))
  })?;
  let state = McpServerState {
    pid: std::process::id(),
    host: host.to_string(),
    port,
    transport: "sse".to_string(),
    endpoint_url: format!("http://{host}:{port}/sse"),
    started_at_utc: Utc::now().to_rfc3339(),
    log_path: log_path(workspace_dir),
  };
  let encoded = serde_json::to_string_pretty(&state)
    .map_err(|error| mcp_error(format!("failed to serialize MCP state: {error}")))?;
  fs::write(state_path(workspace_dir), encoded)
    .map_err(|error| mcp_error(format!("failed to write MCP state: {error}")))?;
  Ok(state)
}

/// Reads recorded MCP server state.
///
/// # Errors
///
/// Returns [`AppError`] when the state file exists but cannot be parsed.
pub fn read_background_state(workspace_dir: &Path) -> Result<Option<McpServerState>, AppError> {
  let path = state_path(workspace_dir);
  if !path.is_file() {
    return Ok(None);
  }
  let content = fs::read_to_string(&path).map_err(|error| {
    mcp_error(format!(
      "failed to read MCP state {}: {error}",
      path.display()
    ))
  })?;
  serde_json::from_str(&content).map(Some).map_err(|error| {
    mcp_error(format!(
      "failed to parse MCP state {}: {error}",
      path.display()
    ))
  })
}

/// Removes recorded MCP server state.
///
/// # Errors
///
/// Returns [`AppError`] when no server is recorded or the state file cannot be removed.
pub fn stop_background_state(workspace_dir: &Path) -> Result<McpServerState, AppError> {
  let state = read_background_state(workspace_dir)?
    .ok_or_else(|| mcp_error("No MCP server is currently running."))?;
  fs::remove_file(state_path(workspace_dir))
    .map_err(|error| mcp_error(format!("failed to remove MCP state: {error}")))?;
  Ok(state)
}

fn limit_or_default(limit: Option<usize>, default_limit: usize) -> Result<usize, AppError> {
  let limit = limit.unwrap_or(default_limit);
  if limit == 0 {
    Err(mcp_error("limit must be greater than 0"))
  } else {
    Ok(limit)
  }
}

fn text_content(text: &str) -> Value {
  json!([{ "type": "text", "text": text }])
}

fn results_to_json(results: &[SearchResult]) -> Result<String, AppError> {
  serde_json::to_string(results).map_err(|error| mcp_error(error.to_string()))
}

fn filtered_search(
  config: &McpConfig,
  query: &str,
  limit: Option<usize>,
  record_type: RecordType,
) -> Result<String, AppError> {
  let limit = limit_or_default(limit, config.default_limit)?;
  let results = retrieval::run_retrieval(&config.retrieval, query, limit)?
    .into_iter()
    .filter(|result| result.record_type == record_type)
    .take(limit)
    .collect::<Vec<_>>();
  results_to_json(&results)
}

/// Returns the stable MCP tool names exposed by `rkb mcp`.
#[must_use]
pub fn tool_names() -> Vec<&'static str> {
  TOOLS.to_vec()
}

/// Calls one read-only MCP tool and returns a JSON string payload.
///
/// # Errors
///
/// Returns [`AppError`] when the tool name, arguments, or retrieval operation is invalid.
pub fn call_tool(
  config: &McpConfig,
  tool_name: &str,
  query: &str,
  limit: Option<usize>,
) -> Result<String, AppError> {
  match tool_name {
    "search_datasets" => filtered_search(config, query, limit, RecordType::Dataset),
    "search_documents" => filtered_search(config, query, limit, RecordType::Document),
    "search_variables" => filtered_search(config, query, limit, RecordType::Variable),
    "search_chunks" => filtered_search(config, query, limit, RecordType::Chunk),
    "get_agent_context" => {
      let limit = limit_or_default(limit, config.default_limit)?;
      let results = retrieval::run_retrieval(&config.retrieval, query, limit)?;
      let context = agent_context::build_agent_context(query, results);
      serde_json::to_string(&context).map_err(|error| mcp_error(error.to_string()))
    }
    _ => Err(mcp_error(format!("unknown MCP tool: {tool_name}"))),
  }
}

fn tool_schema(name: &str) -> Value {
  json!({
    "name": name,
    "description": match name {
      "get_agent_context" => "Return citation-preserving retrieval context.",
      "search_datasets" => "Search dataset records.",
      "search_documents" => "Search document records.",
      "search_variables" => "Search variable records.",
      "search_chunks" => "Search parsed chunk records.",
      _ => "Search records.",
    },
    "inputSchema": {
      "type": "object",
      "properties": {
        "query": { "type": "string" },
        "limit": { "type": "integer", "minimum": 1 }
      },
      "required": ["query"]
    }
  })
}

fn jsonrpc_result(id: &Value, result: &Value) -> Value {
  json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn jsonrpc_error(id: &Value, code: i64, message: impl Into<String>) -> Value {
  json!({
    "jsonrpc": "2.0",
    "id": id,
    "error": { "code": code, "message": message.into() }
  })
}

fn string_arg(arguments: &Value, field: &str) -> Result<String, AppError> {
  arguments
    .get(field)
    .and_then(Value::as_str)
    .map(str::to_string)
    .ok_or_else(|| mcp_error(format!("missing string argument: {field}")))
}

fn limit_arg(arguments: &Value) -> Result<Option<usize>, AppError> {
  arguments
    .get("limit")
    .map(|value| {
      value
        .as_u64()
        .ok_or_else(|| mcp_error("limit must be a positive integer"))
        .and_then(|limit| {
          usize::try_from(limit).map_err(|error| mcp_error(format!("limit is invalid: {error}")))
        })
    })
    .transpose()
}

fn handle_request(config: &McpConfig, request: &Value) -> Option<Value> {
  let id = request.get("id").cloned()?;
  let method = request.get("method").and_then(Value::as_str).unwrap_or("");
  match method {
    "initialize" => Some(jsonrpc_result(
      &id,
      &json!({
        "protocolVersion": "2025-11-25",
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "rkb", "version": env!("CARGO_PKG_VERSION") }
      }),
    )),
    "tools/list" => Some(jsonrpc_result(
      &id,
      &json!({ "tools": TOOLS.iter().map(|tool| tool_schema(tool)).collect::<Vec<_>>() }),
    )),
    "tools/call" => {
      let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
      let name = params.get("name").and_then(Value::as_str).unwrap_or("");
      let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
      let result = string_arg(&arguments, "query")
        .and_then(|query| limit_arg(&arguments).map(|limit| (query, limit)))
        .and_then(|(query, limit)| call_tool(config, name, &query, limit));
      match result {
        Ok(payload) => Some(jsonrpc_result(
          &id,
          &json!({ "content": text_content(&payload) }),
        )),
        Err(error) => Some(jsonrpc_error(&id, -32_002, error.to_string())),
      }
    }
    _ => Some(jsonrpc_error(
      &id,
      -32_601,
      format!("method not found: {method}"),
    )),
  }
}

/// Runs the line-delimited stdio server loop.
///
/// # Errors
///
/// Returns [`AppError`] when input cannot be parsed or output cannot be written.
pub fn run_stdio_server<R, W>(config: &McpConfig, reader: R, mut writer: W) -> Result<(), AppError>
where
  R: BufRead,
  W: Write,
{
  for line in reader.lines() {
    let line = line.map_err(|error| mcp_error(format!("failed to read MCP input: {error}")))?;
    if line.trim().is_empty() {
      continue;
    }
    let request = serde_json::from_str::<Value>(&line)
      .map_err(|error| mcp_error(format!("failed to parse MCP JSON-RPC request: {error}")))?;
    if let Some(response) = handle_request(config, &request) {
      serde_json::to_writer(&mut writer, &response)
        .map_err(|error| mcp_error(format!("failed to write MCP response: {error}")))?;
      writer
        .write_all(b"\n")
        .map_err(|error| mcp_error(format!("failed to write MCP response: {error}")))?;
      writer
        .flush()
        .map_err(|error| mcp_error(format!("failed to flush MCP response: {error}")))?;
    }
  }
  Ok(())
}
