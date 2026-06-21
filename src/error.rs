//! Typed application failures surfaced by the CLI boundary.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Recoverable failures returned by command dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppError {
  /// The command name is stable, but its behavior has not passed parity review.
  CommandUnavailable { command: &'static str },
}

impl Display for AppError {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    match self {
      Self::CommandUnavailable { command } => {
        write!(
          formatter,
          "'{command}' is reserved but not implemented; see SPEC.md"
        )
      }
    }
  }
}

impl Error for AppError {}
