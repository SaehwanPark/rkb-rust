//! Shared contracts for the `rkb` command-line program.
//!
//! The foundation release reserves the CLI namespace without claiming that
//! pipeline behavior has been ported. Each command will replace its explicit
//! unavailable result with a tested implementation in a later rewrite slice.

pub mod cli;
pub mod error;

use cli::Command;
use error::AppError;

/// Dispatches one parsed command.
///
/// # Errors
///
/// Returns [`AppError::CommandUnavailable`] until the selected command has a
/// verified Rust implementation.
pub const fn run(command: Command) -> Result<(), AppError> {
  Err(AppError::CommandUnavailable {
    command: command.name(),
  })
}
