//! Typed command-line parsing for the single `rkb` executable.

use clap::{Parser, Subcommand};

/// ResDAC/CMS documentation knowledge-base tools.
#[derive(Debug, Parser)]
#[command(name = "rkb", version, about)]
pub struct Cli {
  /// Knowledge-base action to run.
  #[command(subcommand)]
  pub command: Command,
}

/// Stable command namespace reserved for the Rust rewrite.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Subcommand)]
pub enum Command {
  /// Discover source pages and assets.
  Inventory,
  /// Preserve discovered sources with provenance.
  Archive,
  /// Extract metadata and graph seeds.
  Extract,
  /// Parse archived documents into provenance-bearing chunks.
  Parse,
  /// Extract variable-level metadata.
  Variables,
  /// Validate provenance and cross-artifact integrity.
  Qa,
  /// Build the derived retrieval index.
  Index,
  /// Search indexed knowledge-base records.
  Search,
  /// Return citation-preserving context for agents.
  AgentContext,
  /// Serve read-only Model Context Protocol tools.
  Mcp,
  /// Configure a local MCP client integration.
  McpSetup,
  /// Evaluate retrieval quality against benchmark questions.
  Evaluate,
  /// Summarize progress from long-running operations.
  Progress,
  /// Run downstream integration helpers.
  Integration,
}

impl Command {
  /// Returns the stable CLI spelling for this command.
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::Inventory => "inventory",
      Self::Archive => "archive",
      Self::Extract => "extract",
      Self::Parse => "parse",
      Self::Variables => "variables",
      Self::Qa => "qa",
      Self::Index => "index",
      Self::Search => "search",
      Self::AgentContext => "agent-context",
      Self::Mcp => "mcp",
      Self::McpSetup => "mcp-setup",
      Self::Evaluate => "evaluate",
      Self::Progress => "progress",
      Self::Integration => "integration",
    }
  }
}
