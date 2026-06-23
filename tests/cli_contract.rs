use std::process::{Command, Output};

const RESERVED_COMMANDS: &[&str] = &[];

const ALL_COMMANDS: &[&str] = &[
  "inventory",
  "archive",
  "extract",
  "parse",
  "variables",
  "qa",
  "index",
  "search",
  "agent-context",
  "mcp",
  "mcp-setup",
  "evaluate",
  "progress",
  "integration",
];

fn run_rkb(args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_rkb"))
    .args(args)
    .output()
    .expect("rkb should execute")
}

#[test]
fn help_lists_every_implemented_command() {
  let output = run_rkb(&["--help"]);
  let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");

  assert!(output.status.success());
  assert!(output.stderr.is_empty());
  for command in ALL_COMMANDS {
    assert!(stdout.contains(command), "help omitted {command}");
  }
}

#[test]
fn version_identifies_the_rkb_program() {
  let output = run_rkb(&["--version"]);
  let stdout = String::from_utf8(output.stdout).expect("version should be UTF-8");

  assert!(output.status.success());
  assert!(output.stderr.is_empty());
  assert!(stdout.starts_with("rkb "));
}

#[test]
fn reserved_commands_fail_deterministically() {
  for command in RESERVED_COMMANDS {
    let output = run_rkb(&[command]);
    let stderr = String::from_utf8(output.stderr).expect("error should be UTF-8");

    assert_eq!(
      output.status.code(),
      Some(1),
      "unexpected status for {command}"
    );
    assert!(output.stdout.is_empty(), "unexpected stdout for {command}");
    assert_eq!(
      stderr,
      format!("rkb: '{command}' is reserved but not implemented; see SPEC.md\n")
    );
  }
}

#[test]
fn archive_accepts_documented_flags() {
  let output = run_rkb(&[
    "archive",
    "--retry-failed-only",
    "--max-downloads",
    "1",
    "--rate-limit-cooldown-seconds",
    "5",
    "--help",
  ]);
  let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");

  assert!(output.status.success(), "archive flags should parse");
  assert!(stdout.contains("--retry-failed-only"));
  assert!(stdout.contains("--max-downloads"));
  assert!(stdout.contains("--rate-limit-cooldown-seconds"));
}

#[test]
fn inventory_accepts_documented_flags() {
  let output = run_rkb(&[
    "inventory",
    "--max-listing-pages",
    "2",
    "--max-follow-pages",
    "10",
    "--help",
  ]);
  let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");

  assert!(output.status.success(), "inventory flags should parse");
  assert!(stdout.contains("--max-pages"));
  assert!(stdout.contains("--max-follow-pages"));
}

#[test]
fn archive_help_documents_retry_failed_only() {
  let output = run_rkb(&["archive", "--help"]);
  let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");

  assert!(output.status.success());
  assert!(stdout.contains("--retry-failed-only"));
  assert!(stdout.contains("previous archive manifest"));
}

#[test]
fn archive_retry_example_flags_parse_without_help() {
  let output = run_rkb(&[
    "archive",
    "--retry-failed-only",
    "--max-downloads",
    "50",
    "--request-delay-seconds",
    "5",
    "--rate-limit-cooldown-seconds",
    "300",
    "--inventory",
    "/nonexistent/rkb-cli-parity-inventory.csv",
  ]);
  let stderr = String::from_utf8_lossy(&output.stderr);

  assert!(
    !stderr.contains("unexpected argument"),
    "retry example flags should parse: {stderr}"
  );
}

#[test]
fn top_level_flags_remain_invalid() {
  let output = run_rkb(&["--retry-failed-only"]);
  let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");

  assert!(!output.status.success());
  assert!(stderr.contains("--retry-failed-only"));
}
