//! MCP client configuration helpers.

use crate::error::AppError;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

fn setup_error(message: impl Into<String>) -> AppError {
  AppError::ConfigValidationError(message.into())
}

/// Returns the default config path for a supported MCP client.
#[must_use]
pub fn default_config_path(client: &str, project_path: &Path, home: &Path) -> Option<PathBuf> {
  match client {
    "claude-desktop" => {
      if cfg!(target_os = "macos") {
        Some(
          home
            .join("Library")
            .join("Application Support")
            .join("Claude")
            .join("claude_desktop_config.json"),
        )
      } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
          .map(PathBuf::from)
          .map(|path| path.join("Claude").join("claude_desktop_config.json"))
      } else {
        Some(
          home
            .join(".config")
            .join("Claude")
            .join("claude_desktop_config.json"),
        )
      }
    }
    "claude-code-project" => Some(project_path.join(".mcp.json")),
    "claude-code-user" => Some(home.join(".claude.json")),
    "antigravity" => Some(
      project_path
        .join(".gemini")
        .join("antigravity-cli")
        .join("mcp_config.json"),
    ),
    "codex-project" => Some(project_path.join(".codex").join("config.toml")),
    _ => None,
  }
}

fn server_json(command: &str, args: &[String]) -> Value {
  json!({ "command": command, "args": args })
}

/// Updates an MCP JSON config file.
///
/// # Errors
///
/// Returns [`AppError`] when the file cannot be read, parsed, or written.
pub fn update_json_config(
  path: &Path,
  server_name: &str,
  command: &str,
  args: &[String],
  dry_run: bool,
) -> Result<String, AppError> {
  let mut root = if path.is_file() {
    let content = fs::read_to_string(path)
      .map_err(|error| setup_error(format!("failed to read {}: {error}", path.display())))?;
    serde_json::from_str::<Value>(&content)
      .map_err(|error| setup_error(format!("invalid JSON in {}: {error}", path.display())))?
  } else {
    json!({})
  };
  let object = root
    .as_object_mut()
    .ok_or_else(|| setup_error(format!("{} must contain a JSON object", path.display())))?;
  let servers = object
    .entry("mcpServers")
    .or_insert_with(|| Value::Object(Map::new()));
  let servers = servers.as_object_mut().ok_or_else(|| {
    setup_error(format!(
      "Existing 'mcpServers' key in {} is not an object",
      path.display()
    ))
  })?;
  let entry = server_json(command, args);
  if servers.get(server_name) == Some(&entry) {
    return Ok(format!(
      "Already configured and up-to-date in {}.",
      path.display()
    ));
  }
  servers.insert(server_name.to_string(), entry);
  if dry_run {
    return Ok(format!(
      "[DRY-RUN] Would configure MCP server in {}.",
      path.display()
    ));
  }
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)
      .map_err(|error| setup_error(format!("failed to create {}: {error}", parent.display())))?;
  }
  let encoded = serde_json::to_string_pretty(&root)
    .map_err(|error| setup_error(format!("failed to serialize JSON config: {error}")))?;
  fs::write(path, format!("{encoded}\n"))
    .map_err(|error| setup_error(format!("failed to write {}: {error}", path.display())))?;
  Ok(format!(
    "Successfully configured MCP server in {}.",
    path.display()
  ))
}

fn toml_section(server_name: &str, command: &str, args: &[String]) -> String {
  let args = args
    .iter()
    .map(|arg| format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\"")))
    .collect::<Vec<_>>()
    .join(", ");
  format!(
    "[mcp_servers.{server_name}]\ntype = \"stdio\"\ncommand = \"{}\"\nargs = [{args}]\n",
    command.replace('\\', "\\\\").replace('"', "\\\"")
  )
}

fn is_section_header(line: &str) -> bool {
  line.trim_start().starts_with('[')
}

fn is_server_header(line: &str, server_name: &str) -> bool {
  let trimmed = line.trim();
  trimmed == format!("[mcp_servers.{server_name}]")
    || trimmed == format!("[mcp_servers.\"{server_name}\"]")
}

/// Updates or appends a TOML MCP server section.
#[must_use]
pub fn update_toml_string(
  existing: &str,
  server_name: &str,
  command: &str,
  args: &[String],
) -> String {
  let replacement = toml_section(server_name, command, args);
  let lines = existing.lines().collect::<Vec<_>>();
  let Some(start) = lines
    .iter()
    .position(|line| is_server_header(line, server_name))
  else {
    let separator = if existing.trim().is_empty() {
      ""
    } else {
      "\n\n"
    };
    return format!("{}{}{}", existing.trim_end(), separator, replacement);
  };
  let end = lines[start + 1..]
    .iter()
    .position(|line| is_section_header(line))
    .map_or(lines.len(), |offset| start + 1 + offset);
  let mut preserved = lines[start + 1..end]
    .iter()
    .filter(|line| {
      let trimmed = line.trim_start();
      trimmed.starts_with('#')
        || (!trimmed.starts_with("type ")
          && !trimmed.starts_with("command ")
          && !trimmed.starts_with("args "))
    })
    .map(|line| (*line).to_string())
    .collect::<Vec<_>>();
  let mut output = Vec::new();
  output.extend(lines[..start].iter().map(|line| (*line).to_string()));
  output.extend(replacement.trim_end().lines().map(str::to_string));
  output.append(&mut preserved);
  output.extend(lines[end..].iter().map(|line| (*line).to_string()));
  format!("{}\n", output.join("\n").trim_end())
}

/// Updates an MCP TOML config file.
///
/// # Errors
///
/// Returns [`AppError`] when the file cannot be read or written.
pub fn update_toml_config(
  path: &Path,
  server_name: &str,
  command: &str,
  args: &[String],
  dry_run: bool,
) -> Result<String, AppError> {
  let existing = if path.is_file() {
    fs::read_to_string(path)
      .map_err(|error| setup_error(format!("failed to read {}: {error}", path.display())))?
  } else {
    String::new()
  };
  let updated = update_toml_string(&existing, server_name, command, args);
  if dry_run {
    return Ok(format!(
      "[DRY-RUN] Would configure MCP server in {}.",
      path.display()
    ));
  }
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)
      .map_err(|error| setup_error(format!("failed to create {}: {error}", parent.display())))?;
  }
  fs::write(path, updated)
    .map_err(|error| setup_error(format!("failed to write {}: {error}", path.display())))?;
  Ok(format!(
    "Successfully configured MCP server in {}.",
    path.display()
  ))
}

/// Configures one supported MCP client.
///
/// # Errors
///
/// Returns [`AppError`] for unknown clients or failed file updates.
pub fn configure_client(
  client: &str,
  config_path: Option<&Path>,
  project_path: &Path,
  server_name: &str,
  command: &str,
  dry_run: bool,
) -> Result<String, AppError> {
  let home = std::env::var_os("HOME").map_or_else(|| project_path.to_path_buf(), PathBuf::from);
  let path = config_path
    .map(Path::to_path_buf)
    .or_else(|| default_config_path(client, project_path, &home))
    .ok_or_else(|| setup_error(format!("unsupported MCP client: {client}")))?;
  let args = vec!["mcp".to_string()];
  if path.extension().and_then(|extension| extension.to_str()) == Some("toml") {
    update_toml_config(&path, server_name, command, &args, dry_run)
  } else {
    update_json_config(&path, server_name, command, &args, dry_run)
  }
}
