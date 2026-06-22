use std::process::{Command, Output};

const RESERVED_COMMANDS: &[&str] = &[
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
fn help_lists_every_reserved_command() {
  let output = run_rkb(&["--help"]);
  let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");

  assert!(output.status.success());
  assert!(output.stderr.is_empty());
  for command in RESERVED_COMMANDS {
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
