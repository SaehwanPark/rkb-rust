#![allow(clippy::collapsible_if)]

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::AppError;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProgressLogSummary {
  pub path: PathBuf,
  pub event_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProgressSummary {
  pub log_count: usize,
  pub event_count: usize,
  pub stages: BTreeMap<String, usize>,
  pub events: BTreeMap<String, usize>,
  pub latest_event: Option<ProgressEvent>,
  pub latest_counts: BTreeMap<String, usize>,
  pub logs: Vec<ProgressLogSummary>,
}

#[must_use]
pub fn now_utc_timestamp() -> String {
  // Format matching Python: '2026-06-19T01:51:00.000000Z'
  Utc::now().format("%Y-%m-%dT%H:%M:%S.%6fZ").to_string()
}

fn read_progress_log(path: &Path) -> Result<Vec<ProgressEvent>, AppError> {
  let content = fs::read_to_string(path).map_err(|error| {
    AppError::RecordParseError(format!(
      "failed to read progress log {}: {error}",
      path.display()
    ))
  })?;
  let mut events = Vec::new();
  for (index, line) in content.lines().enumerate() {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      continue;
    }
    let event = serde_json::from_str::<ProgressEvent>(trimmed).map_err(|error| {
      AppError::RecordParseError(format!(
        "failed to parse progress log {} line {}: {error}",
        path.display(),
        index + 1
      ))
    })?;
    events.push(event);
  }
  Ok(events)
}

#[must_use]
pub fn default_progress_log_paths() -> Vec<PathBuf> {
  vec![
    PathBuf::from("_workspace/02_inventory_progress.jsonl"),
    PathBuf::from("_workspace/03_archive_progress.jsonl"),
  ]
}

/// Summarizes one or more progress JSONL logs.
///
/// # Errors
///
/// Returns an error when an explicit log path is missing, no default logs are
/// found, a log cannot be read, or a non-empty JSONL line cannot be parsed as a
/// [`ProgressEvent`].
pub fn summarize_progress_logs(
  paths: &[PathBuf],
  explicit: bool,
) -> Result<ProgressSummary, AppError> {
  let mut logs = Vec::new();
  let mut all_events = Vec::new();
  for path in paths {
    if !path.exists() {
      if explicit {
        return Err(AppError::RecordParseError(format!(
          "progress log does not exist: {}",
          path.display()
        )));
      }
      continue;
    }
    let events = read_progress_log(path)?;
    logs.push(ProgressLogSummary {
      path: path.clone(),
      event_count: events.len(),
    });
    all_events.extend(events);
  }

  if !explicit && logs.is_empty() {
    return Err(AppError::RecordParseError(
      "no default progress logs found; pass --log <PATH>".to_string(),
    ));
  }

  Ok(summarize_progress_events(logs, &all_events))
}

#[must_use]
pub fn summarize_progress_events(
  logs: Vec<ProgressLogSummary>,
  events: &[ProgressEvent],
) -> ProgressSummary {
  let mut stages = BTreeMap::new();
  let mut event_names = BTreeMap::new();
  for event in events {
    *stages.entry(event.stage.clone()).or_insert(0) += 1;
    *event_names.entry(event.event.clone()).or_insert(0) += 1;
  }
  let latest_event = events
    .iter()
    .enumerate()
    .max_by(|(left_index, left), (right_index, right)| {
      left
        .timestamp_utc
        .cmp(&right.timestamp_utc)
        .then_with(|| left_index.cmp(right_index))
    })
    .map(|(_, event)| event.clone());
  let latest_counts = latest_event.as_ref().map_or_else(BTreeMap::new, |event| {
    event
      .counts
      .iter()
      .map(|(key, value)| (key.clone(), *value))
      .collect()
  });

  ProgressSummary {
    log_count: logs.len(),
    event_count: events.len(),
    stages,
    events: event_names,
    latest_event,
    latest_counts,
    logs,
  }
}

#[must_use]
pub fn format_progress_summary_text(summary: &ProgressSummary) -> String {
  let mut lines = vec![
    "Progress summary".to_string(),
    format!("Logs: {}", summary.log_count),
    format!("Events: {}", summary.event_count),
  ];

  if summary.event_count == 0 {
    lines.push("No progress events found.".to_string());
    return lines.join("\n");
  }

  lines.push("Stages:".to_string());
  for (stage, count) in &summary.stages {
    lines.push(format!("- {stage}: {count}"));
  }
  lines.push("Event types:".to_string());
  for (event, count) in &summary.events {
    lines.push(format!("- {event}: {count}"));
  }
  if let Some(event) = &summary.latest_event {
    lines.push(format!(
      "Latest: {} {} {}",
      event.timestamp_utc, event.stage, event.event
    ));
    if !event.message.is_empty() {
      lines.push(format!("Message: {}", event.message));
    }
  }
  if !summary.latest_counts.is_empty() {
    lines.push("Latest counts:".to_string());
    for (key, value) in &summary.latest_counts {
      lines.push(format!("- {key}: {value}"));
    }
  }
  lines.join("\n")
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
