use rkb::mcp_setup::{
  default_config_path, update_json_config, update_toml_config, update_toml_string,
};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temp_root() -> std::path::PathBuf {
  let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
  std::env::temp_dir().join(format!("rkb-mcp-setup-{}-{sequence}", std::process::id()))
}

fn args() -> Vec<String> {
  vec!["mcp".to_string()]
}

#[test]
fn detects_supported_default_config_paths() {
  let root = temp_root();
  assert_eq!(
    default_config_path("claude-code-project", &root, &root),
    Some(root.join(".mcp.json"))
  );
  assert_eq!(
    default_config_path("claude-code-user", &root, &root),
    Some(root.join(".claude.json"))
  );
  assert_eq!(
    default_config_path("antigravity", &root, &root),
    Some(
      root
        .join(".gemini")
        .join("antigravity-cli")
        .join("mcp_config.json")
    )
  );
  assert_eq!(
    default_config_path("codex-project", &root, &root),
    Some(root.join(".codex").join("config.toml"))
  );
  assert_eq!(default_config_path("unknown", &root, &root), None);
}

#[test]
fn updates_json_config_and_preserves_existing_servers() {
  let root = temp_root();
  let path = root.join("config.json");
  fs::create_dir_all(path.parent().unwrap()).unwrap();
  fs::write(
    &path,
    r#"{"other_key":"value","mcpServers":{"other":{"command":"node","args":[]}}}"#,
  )
  .unwrap();

  let message = update_json_config(&path, "rkb", "rkb", &args(), false).unwrap();
  assert!(message.contains("Successfully configured"));
  let payload: serde_json::Value =
    serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
  assert_eq!(payload["other_key"], "value");
  assert_eq!(payload["mcpServers"]["other"]["command"], "node");
  assert_eq!(payload["mcpServers"]["rkb"]["command"], "rkb");
  assert_eq!(payload["mcpServers"]["rkb"]["args"][0], "mcp");

  let second = update_json_config(&path, "rkb", "rkb", &args(), false).unwrap();
  assert!(second.contains("Already configured"));
}

#[test]
fn rejects_invalid_json_config() {
  let root = temp_root();
  let path = root.join("config.json");
  fs::create_dir_all(path.parent().unwrap()).unwrap();
  fs::write(&path, "not json").unwrap();

  assert!(
    update_json_config(&path, "rkb", "rkb", &args(), false)
      .unwrap_err()
      .to_string()
      .contains("invalid JSON")
  );
}

#[test]
fn updates_toml_string_and_preserves_custom_lines() {
  let existing = concat!(
    "[mcp_servers.\"rkb\"]\n",
    "type = \"stdio\"\n",
    "# keep this\n",
    "command = \"old\"\n",
    "args = [\"old\"]\n",
    "command_timeout = 30\n",
    "\n",
    "[other]\n",
    "key = 1\n"
  );
  let updated = update_toml_string(existing, "rkb", "rkb", &args());

  assert!(updated.contains("[mcp_servers.rkb]"));
  assert!(updated.contains("command = \"rkb\""));
  assert!(updated.contains("args = [\"mcp\"]"));
  assert!(updated.contains("# keep this"));
  assert!(updated.contains("command_timeout = 30"));
  assert!(updated.contains("[other]"));
  assert!(!updated.contains("command = \"old\""));
}

#[test]
fn update_toml_config_supports_dry_run() {
  let root = temp_root();
  let path = root.join("config.toml");
  let message = update_toml_config(&path, "rkb", "rkb", &args(), true).unwrap();

  assert!(message.contains("[DRY-RUN]"));
  assert!(!Path::new(&path).exists());
}

#[test]
fn mcp_setup_cli_configures_project_json_and_requires_client() {
  let root = temp_root();
  let missing = Command::new(env!("CARGO_BIN_EXE_rkb"))
    .arg("mcp-setup")
    .output()
    .expect("mcp-setup should run");
  assert_eq!(missing.status.code(), Some(1));
  assert!(
    String::from_utf8(missing.stderr)
      .unwrap()
      .contains("at least one --client is required")
  );

  let output = Command::new(env!("CARGO_BIN_EXE_rkb"))
    .args([
      "mcp-setup",
      "--client",
      "claude-code-project",
      "--project-path",
      root.to_str().unwrap(),
    ])
    .output()
    .expect("mcp-setup should run");
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(root.join(".mcp.json").is_file());
}
