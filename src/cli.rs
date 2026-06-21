//! Typed command-line parsing for the single `rkb` executable.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// ResDAC/CMS documentation knowledge-base tools.
#[derive(Debug, Parser)]
#[command(name = "rkb", version, about)]
pub struct Cli {
  /// Knowledge-base action to run.
  #[command(subcommand)]
  pub command: Command,
}

/// Arguments for the `inventory` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct InventoryArgs {
  #[arg(long, default_value = "https://resdac.org/cms-data")]
  pub base_url: String,

  #[arg(long, default_value_t = 4, aliases = ["max-listing-pages"])]
  pub max_pages: usize,

  #[arg(long)]
  pub max_follow_pages: Option<usize>,

  #[arg(long)]
  pub max_assets: Option<usize>,

  #[arg(long, default_value = "manifests/site_inventory.csv")]
  pub output: PathBuf,

  #[arg(long, default_value = "manifests/site_inventory_edges.csv")]
  pub edge_output: PathBuf,

  #[arg(long, default_value = "_workspace")]
  pub workspace_dir: PathBuf,

  #[arg(long, default_value = "_workspace/02_inventory_progress.jsonl")]
  pub progress_log: PathBuf,

  #[arg(long)]
  pub no_progress_log: bool,

  #[arg(long, default_value_t = 20.0)]
  pub timeout_seconds: f64,

  #[arg(long, default_value_t = 0.5)]
  pub request_delay_seconds: f64,

  #[arg(long, default_value_t = 25)]
  pub progress_interval: usize,
}

/// Arguments for the `archive` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct ArchiveArgs {
  #[arg(long, default_value = "manifests/site_inventory.csv")]
  pub inventory: PathBuf,

  #[arg(long, default_value = "data/raw")]
  pub raw_root: PathBuf,

  #[arg(long, default_value = "manifests/archive_manifest.csv")]
  pub manifest_output: PathBuf,

  #[arg(long, default_value = "_workspace")]
  pub workspace_dir: PathBuf,

  #[arg(long, default_value_t = 20.0)]
  pub timeout_seconds: f64,

  #[arg(long, default_value_t = 0.5)]
  pub request_delay_seconds: f64,

  #[arg(long, default_value_t = 5)]
  pub max_consecutive_rate_limits: usize,

  #[arg(long)]
  pub retry_failed_only: bool,

  #[arg(long)]
  pub max_downloads: Option<usize>,

  #[arg(long, default_value_t = 0.0)]
  pub rate_limit_cooldown_seconds: f64,

  #[arg(long, default_value = "_workspace/03_archive_progress.jsonl")]
  pub progress_log: PathBuf,

  #[arg(long)]
  pub no_progress_log: bool,

  #[arg(long, default_value_t = 25)]
  pub progress_interval: usize,
}

/// Stable command namespace reserved for the Rust rewrite.
#[derive(Clone, Debug, PartialEq, Subcommand)]
pub enum Command {
  /// Discover source pages and assets.
  Inventory(InventoryArgs),
  /// Preserve discovered sources with provenance.
  Archive(ArchiveArgs),
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
  pub const fn name(&self) -> &'static str {
    match self {
      Self::Inventory(_) => "inventory",
      Self::Archive(_) => "archive",
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
