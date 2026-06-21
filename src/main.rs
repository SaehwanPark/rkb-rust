use std::process::ExitCode;

use clap::Parser;
use rkb::cli::Cli;

fn main() -> ExitCode {
  let cli = Cli::parse();

  match rkb::run(cli.command) {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("rkb: {error}");
      ExitCode::FAILURE
    }
  }
}
