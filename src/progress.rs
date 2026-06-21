#![allow(clippy::collapsible_if)]

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgressEvent {
  pub timestamp_utc: String,
  pub stage: String,
  pub event: String,
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub message: String,
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub url: String,
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub resource_kind: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub status: Option<u16>,
  #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  pub counts: HashMap<String, usize>,
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub error: String,
}

#[must_use]
pub fn now_utc_timestamp() -> String {
  // Format matching Python: '2026-06-19T01:51:00.000000Z'
  Utc::now().format("%Y-%m-%dT%H:%M:%S.%6fZ").to_string()
}

pub fn init_progress_log(log_path: &Path) {
  if let Some(parent) = log_path.parent() {
    if let Err(e) = fs::create_dir_all(parent) {
      eprintln!(
        "warning: failed to create parent directories for progress log {}: {}",
        log_path.display(),
        e
      );
    }
  }
  if let Err(e) = fs::write(log_path, "") {
    eprintln!(
      "warning: failed to initialize progress log {}: {}",
      log_path.display(),
      e
    );
  }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::implicit_hasher)]
pub fn append_progress_event(
  log_path: Option<&Path>,
  stage: &str,
  event: &str,
  message: &str,
  url: &str,
  resource_kind: &str,
  status: Option<u16>,
  counts: Option<HashMap<String, usize>>,
  error: &str,
) {
  let Some(path) = log_path else {
    return;
  };
  if let Some(parent) = path.parent() {
    if let Err(e) = fs::create_dir_all(parent) {
      eprintln!(
        "warning: failed to create parent directories for progress log {}: {}",
        path.display(),
        e
      );
    }
  }
  let progress_event = ProgressEvent {
    timestamp_utc: now_utc_timestamp(),
    stage: stage.to_string(),
    event: event.to_string(),
    message: message.to_string(),
    url: url.to_string(),
    resource_kind: resource_kind.to_string(),
    status,
    counts: counts.unwrap_or_default(),
    error: error.to_string(),
  };

  match OpenOptions::new().create(true).append(true).open(path) {
    Ok(mut file) => match serde_json::to_string(&progress_event) {
      Ok(json_str) => {
        if let Err(e) = writeln!(file, "{json_str}") {
          eprintln!(
            "warning: failed to write progress log to {}: {}",
            path.display(),
            e
          );
        }
      }
      Err(e) => {
        eprintln!("warning: failed to serialize progress event to JSON: {e}");
      }
    },
    Err(e) => {
      eprintln!(
        "warning: failed to open progress log file {}: {}",
        path.display(),
        e
      );
    }
  }
}
