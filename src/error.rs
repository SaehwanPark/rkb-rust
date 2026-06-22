//! Typed application failures surfaced by the CLI boundary.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Recoverable failures returned by command dispatch and domain operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppError {
  /// The command name is stable, but its behavior has not passed parity review.
  CommandUnavailable { command: &'static str },
  /// Configuration input is invalid.
  ConfigValidationError(String),
  /// Path resolution failure.
  PathResolutionError(String),
  /// CSV or JSONL record parsing/serialization failure.
  RecordParseError(String),
  /// `SQLite` index construction or retrieval failure.
  RetrievalError(String),
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
      Self::ConfigValidationError(msg) => {
        write!(formatter, "configuration validation error: {msg}")
      }
      Self::PathResolutionError(msg) => {
        write!(formatter, "path resolution error: {msg}")
      }
      Self::RecordParseError(msg) => {
        write!(formatter, "record parse error: {msg}")
      }
      Self::RetrievalError(msg) => {
        write!(formatter, "retrieval error: {msg}")
      }
    }
  }
}

impl Error for AppError {}
