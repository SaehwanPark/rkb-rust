use rkb::progress::{
  ProgressEvent, ProgressLogSummary, format_progress_summary_text, summarize_progress_events,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
  root: PathBuf,
}

impl Drop for Fixture {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.root);
  }
}

fn fixture() -> Fixture {
  let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
  Fixture {
    root: std::env::temp_dir().join(format!("rkb-progress-{}-{sequence}", std::process::id())),
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

fn progress_command(log_path: &Path, json: bool) -> Command {
  let mut command = Command::new(env!("CARGO_BIN_EXE_rkb"));
  command.args(["progress", "--log", path_str(log_path)]);
  if json {
    command.arg("--json");
  }
  command
}

#[test]
fn summarizes_events_with_deterministic_text() {
  let mut counts = HashMap::new();
  counts.insert("processed".to_string(), 2);
  counts.insert("archived".to_string(), 1);
  let summary = summarize_progress_events(
    vec![ProgressLogSummary {
      path: PathBuf::from("progress.jsonl"),
      event_count: 2,
    }],
    &[
      ProgressEvent {
        timestamp_utc: "2026-06-23T10:00:00.000000Z".to_string(),
        stage: "archive".to_string(),
        event: "start".to_string(),
        message: String::new(),
        url: String::new(),
        resource_kind: String::new(),
        status: None,
        counts: HashMap::new(),
        error: String::new(),
      },
      ProgressEvent {
        timestamp_utc: "2026-06-23T10:01:00.000000Z".to_string(),
        stage: "archive".to_string(),
        event: "progress".to_string(),
        message: "processed 2 rows".to_string(),
        url: String::new(),
        resource_kind: String::new(),
        status: None,
        counts,
        error: String::new(),
      },
    ],
  );

  let text = format_progress_summary_text(&summary);
  assert!(text.contains("Progress summary"));
  assert!(text.contains("Logs: 1"));
  assert!(text.contains("Events: 2"));
  assert!(text.contains("- archive: 2"));
  assert!(text.contains("- progress: 1"));
  assert!(text.contains("- start: 1"));
  assert!(text.contains("Latest: 2026-06-23T10:01:00.000000Z archive progress"));
  assert!(
    text
      .find("- archived: 1")
      .expect("archived count should exist")
      < text
        .find("- processed: 2")
        .expect("processed count should exist")
  );
}

#[test]
fn progress_command_emits_text_and_json() {
  let fixture = fixture();
  let log_path = fixture.root.join("progress.jsonl");
  write(
    &log_path,
    "{\"timestamp_utc\":\"2026-06-23T10:00:00.000000Z\",\"stage\":\"inventory\",\"event\":\"start\",\"message\":\"starting\",\"counts\":{\"total_urls\":0}}\n\
     {\"timestamp_utc\":\"2026-06-23T10:02:00.000000Z\",\"stage\":\"inventory\",\"event\":\"complete\",\"message\":\"done\",\"counts\":{\"dead_links\":1,\"total_urls\":3}}\n",
  );

  let text = progress_command(&log_path, false)
    .output()
    .expect("progress text command should execute");
  assert!(
    text.status.success(),
    "{}",
    String::from_utf8_lossy(&text.stderr)
  );
  let stdout = String::from_utf8(text.stdout).expect("text output should be UTF-8");
  assert!(stdout.contains("Progress summary"));
  assert!(stdout.contains("Logs: 1"));
  assert!(stdout.contains("Events: 2"));
  assert!(stdout.contains("- inventory: 2"));
  assert!(stdout.contains("Latest: 2026-06-23T10:02:00.000000Z inventory complete"));

  let json = progress_command(&log_path, true)
    .output()
    .expect("progress JSON command should execute");
  assert!(
    json.status.success(),
    "{}",
    String::from_utf8_lossy(&json.stderr)
  );
  let payload: serde_json::Value =
    serde_json::from_slice(&json.stdout).expect("JSON output should parse");
  assert_eq!(payload["log_count"], 1);
  assert_eq!(payload["event_count"], 2);
  assert_eq!(payload["stages"]["inventory"], 2);
  assert_eq!(payload["events"]["complete"], 1);
  assert_eq!(payload["latest_event"]["event"], "complete");
  assert_eq!(payload["latest_counts"]["dead_links"], 1);
  assert_eq!(payload["logs"][0]["event_count"], 2);
}

#[test]
fn progress_command_uses_latest_timestamp_across_logs() {
  let fixture = fixture();
  let older_log = fixture.root.join("older.jsonl");
  let newer_log = fixture.root.join("newer.jsonl");
  write(
    &older_log,
    "{\"timestamp_utc\":\"2026-06-23T10:00:00.000000Z\",\"stage\":\"archive\",\"event\":\"complete\",\"message\":\"older\",\"counts\":{\"archived\":1}}\n",
  );
  write(
    &newer_log,
    "{\"timestamp_utc\":\"2026-06-23T10:05:00.000000Z\",\"stage\":\"inventory\",\"event\":\"complete\",\"message\":\"newer\",\"counts\":{\"total_urls\":4}}\n",
  );

  let output = Command::new(env!("CARGO_BIN_EXE_rkb"))
    .args([
      "progress",
      "--log",
      path_str(&newer_log),
      "--log",
      path_str(&older_log),
      "--json",
    ])
    .output()
    .expect("progress JSON command should execute");

  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  let payload: serde_json::Value =
    serde_json::from_slice(&output.stdout).expect("JSON output should parse");
  assert_eq!(payload["latest_event"]["stage"], "inventory");
  assert_eq!(payload["latest_event"]["message"], "newer");
  assert_eq!(payload["latest_counts"]["total_urls"], 4);
}

#[test]
fn progress_command_handles_empty_logs_and_bad_inputs() {
  let fixture = fixture();
  let empty_path = fixture.root.join("empty.jsonl");
  write(&empty_path, "");

  let empty = progress_command(&empty_path, false)
    .output()
    .expect("empty progress command should execute");
  assert!(
    empty.status.success(),
    "{}",
    String::from_utf8_lossy(&empty.stderr)
  );
  assert!(
    String::from_utf8(empty.stdout)
      .expect("empty stdout should be UTF-8")
      .contains("No progress events found.")
  );

  let bad_path = fixture.root.join("bad.jsonl");
  write(
    &bad_path,
    "{\"timestamp_utc\":\"2026-06-23T10:00:00.000000Z\",\"stage\":\"inventory\",\"event\":\"start\"}\nnot json\n",
  );
  let bad = progress_command(&bad_path, false)
    .output()
    .expect("bad progress command should execute");
  assert_eq!(bad.status.code(), Some(1));
  let bad_stderr = String::from_utf8(bad.stderr).expect("bad stderr should be UTF-8");
  assert!(bad_stderr.contains("bad.jsonl line 2"));

  let missing_path = fixture.root.join("missing.jsonl");
  let missing = progress_command(&missing_path, false)
    .output()
    .expect("missing progress command should execute");
  assert_eq!(missing.status.code(), Some(1));
  assert!(
    String::from_utf8(missing.stderr)
      .expect("missing stderr should be UTF-8")
      .contains("progress log does not exist")
  );
}

#[test]
fn progress_command_reports_when_default_logs_are_absent() {
  let fixture = fixture();
  fs::create_dir_all(&fixture.root).expect("fixture directory should be created");
  let output = Command::new(env!("CARGO_BIN_EXE_rkb"))
    .current_dir(&fixture.root)
    .arg("progress")
    .output()
    .expect("default progress command should execute");

  assert_eq!(output.status.code(), Some(1));
  assert!(
    String::from_utf8(output.stderr)
      .expect("default stderr should be UTF-8")
      .contains("no default progress logs found")
  );
}
