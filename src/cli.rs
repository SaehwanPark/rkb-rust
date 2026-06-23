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

/// Arguments for the `extract` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct ExtractArgs {
  #[arg(long, default_value = "manifests/archive_manifest.csv")]
  pub archive_manifest: PathBuf,

  #[arg(long, default_value = "data/metadata")]
  pub metadata_dir: PathBuf,

  #[arg(long, default_value = "data/graph")]
  pub graph_dir: PathBuf,

  #[arg(long, default_value = "_workspace")]
  pub workspace_dir: PathBuf,
}

/// Arguments for the `parse` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct ParseArgs {
  #[arg(long, default_value = "data/metadata/datasets.csv")]
  pub datasets_metadata: PathBuf,

  #[arg(long, default_value = "data/metadata/documents.csv")]
  pub documents_metadata: PathBuf,

  #[arg(long, default_value = "data/parsed")]
  pub parsed_root: PathBuf,

  #[arg(long, default_value = "_workspace")]
  pub workspace_dir: PathBuf,

  #[arg(long, default_value_t = 500)]
  pub chunk_size: usize,

  #[arg(long, default_value_t = 100)]
  pub chunk_overlap: usize,
}

/// Arguments for the `variables` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct VariablesArgs {
  #[arg(long, default_value = "data/parsed/chunks.jsonl")]
  pub chunks_jsonl: PathBuf,

  #[arg(long, default_value = "manifests/archive_manifest.csv")]
  pub archive_manifest: PathBuf,

  #[arg(long, default_value = "data/metadata")]
  pub metadata_dir: PathBuf,

  #[arg(long, default_value = "data/graph")]
  pub graph_dir: PathBuf,

  #[arg(long, default_value = "_workspace")]
  pub workspace_dir: PathBuf,
}

/// Arguments for the `qa` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct QaArgs {
  #[arg(long, default_value = "data/metadata/datasets.csv")]
  pub datasets_metadata: PathBuf,
  #[arg(long, default_value = "data/metadata/documents.csv")]
  pub documents_metadata: PathBuf,
  #[arg(long, default_value = "data/metadata/variables.csv")]
  pub variables_metadata: PathBuf,
  #[arg(long, default_value = "data/metadata/canonical_variables.csv")]
  pub canonical_variables_metadata: PathBuf,
  #[arg(long, default_value = "data/graph/document_edges.csv")]
  pub document_edges: PathBuf,
  #[arg(long, default_value = "data/graph/variable_edges.csv")]
  pub variable_edges: PathBuf,
  #[arg(long, default_value = "data/graph/data_source_variable_edges.csv")]
  pub data_source_variable_edges: PathBuf,
  #[arg(long, default_value = "data/graph/ontology_nodes.csv")]
  pub ontology_nodes: PathBuf,
  #[arg(long, default_value = "data/graph/ontology_edges.csv")]
  pub ontology_edges: PathBuf,
  #[arg(long, default_value = "manifests/archive_manifest.csv")]
  pub archive_manifest: PathBuf,
  #[arg(long, default_value = "_workspace")]
  pub workspace_dir: PathBuf,
}

/// Shared artifact paths for lexical indexing and retrieval.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct RetrievalPathsArgs {
  #[arg(long, default_value = "data/metadata/datasets.csv")]
  pub datasets_metadata: PathBuf,
  #[arg(long, default_value = "data/metadata/documents.csv")]
  pub documents_metadata: PathBuf,
  #[arg(long, default_value = "data/metadata/variables.csv")]
  pub variables_metadata: PathBuf,
  #[arg(long, default_value = "data/parsed/chunks.jsonl")]
  pub chunks_jsonl: PathBuf,
  #[arg(long, default_value = "data/index/retrieval.sqlite")]
  pub database_path: PathBuf,
}

impl RetrievalPathsArgs {
  #[must_use]
  pub fn into_config(self) -> crate::config::RetrievalConfig {
    crate::config::RetrievalConfig {
      datasets_metadata_path: self.datasets_metadata,
      documents_metadata_path: self.documents_metadata,
      variables_metadata_path: self.variables_metadata,
      chunks_jsonl_path: self.chunks_jsonl,
      database_path: self.database_path,
      ..crate::config::RetrievalConfig::default()
    }
  }
}

/// Arguments for the `index` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct IndexArgs {
  #[command(flatten)]
  pub paths: RetrievalPathsArgs,
}

/// Arguments for the `search` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct SearchArgs {
  #[arg(long)]
  pub query: String,
  #[arg(long, default_value_t = 10)]
  pub limit: usize,
  #[arg(long)]
  pub json: bool,
  #[command(flatten)]
  pub paths: RetrievalPathsArgs,
}

/// Arguments for the `agent-context` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct AgentContextArgs {
  #[arg(long)]
  pub query: String,
  #[arg(long, default_value_t = crate::config::AgentContextConfig::DEFAULT_LIMIT)]
  pub limit: usize,
  #[arg(long)]
  pub json: bool,
  #[command(flatten)]
  pub paths: RetrievalPathsArgs,
}

/// Arguments for the `progress` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct ProgressArgs {
  #[arg(long = "log")]
  pub logs: Vec<PathBuf>,
  #[arg(long)]
  pub json: bool,
}

impl QaArgs {
  #[must_use]
  pub fn from_config(config: crate::config::QAConfig) -> Self {
    Self {
      datasets_metadata: config.datasets_metadata_path,
      documents_metadata: config.documents_metadata_path,
      variables_metadata: config.variables_metadata_path,
      canonical_variables_metadata: config.canonical_variables_metadata_path,
      document_edges: config.document_edges_path,
      variable_edges: config.variable_edges_path,
      data_source_variable_edges: config.data_source_variable_edges_path,
      ontology_nodes: config.ontology_nodes_path,
      ontology_edges: config.ontology_edges_path,
      archive_manifest: config.archive_manifest_path,
      workspace_dir: config.workspace_dir,
    }
  }

  #[must_use]
  pub fn into_config(mut self) -> crate::config::QAConfig {
    let defaults = crate::config::QAConfig::default();
    if self.datasets_metadata != defaults.datasets_metadata_path {
      let metadata_dir = self
        .datasets_metadata
        .parent()
        .unwrap_or_else(|| std::path::Path::new(""));
      let graph_dir = metadata_dir
        .parent()
        .unwrap_or_else(|| std::path::Path::new(""))
        .join("graph");
      if self.variables_metadata == defaults.variables_metadata_path {
        self.variables_metadata = metadata_dir.join("variables.csv");
      }
      if self.canonical_variables_metadata == defaults.canonical_variables_metadata_path {
        self.canonical_variables_metadata = metadata_dir.join("canonical_variables.csv");
      }
      if self.document_edges == defaults.document_edges_path {
        self.document_edges = graph_dir.join("document_edges.csv");
      }
    }
    if self.document_edges != defaults.document_edges_path {
      let graph_dir = self
        .document_edges
        .parent()
        .unwrap_or_else(|| std::path::Path::new(""));
      if self.variable_edges == defaults.variable_edges_path {
        self.variable_edges = graph_dir.join("variable_edges.csv");
      }
      if self.data_source_variable_edges == defaults.data_source_variable_edges_path {
        self.data_source_variable_edges = graph_dir.join("data_source_variable_edges.csv");
      }
      if self.ontology_nodes == defaults.ontology_nodes_path {
        self.ontology_nodes = graph_dir.join("ontology_nodes.csv");
      }
      if self.ontology_edges == defaults.ontology_edges_path {
        self.ontology_edges = graph_dir.join("ontology_edges.csv");
      }
    }
    crate::config::QAConfig {
      datasets_metadata_path: self.datasets_metadata,
      documents_metadata_path: self.documents_metadata,
      variables_metadata_path: self.variables_metadata,
      canonical_variables_metadata_path: self.canonical_variables_metadata,
      document_edges_path: self.document_edges,
      variable_edges_path: self.variable_edges,
      data_source_variable_edges_path: self.data_source_variable_edges,
      ontology_nodes_path: self.ontology_nodes,
      ontology_edges_path: self.ontology_edges,
      archive_manifest_path: self.archive_manifest,
      workspace_dir: self.workspace_dir,
    }
  }
}

/// Stable command namespace reserved for the Rust rewrite.
#[derive(Clone, Debug, PartialEq, Subcommand)]
pub enum Command {
  /// Discover source pages and assets.
  Inventory(InventoryArgs),
  /// Preserve discovered sources with provenance.
  Archive(ArchiveArgs),
  /// Extract metadata and graph seeds.
  Extract(ExtractArgs),
  /// Parse archived documents into provenance-bearing chunks.
  Parse(ParseArgs),
  /// Extract variable-level metadata.
  Variables(VariablesArgs),
  /// Validate provenance and cross-artifact integrity.
  Qa(QaArgs),
  /// Build the derived retrieval index.
  Index(IndexArgs),
  /// Search indexed knowledge-base records.
  Search(SearchArgs),
  /// Return citation-preserving context for agents.
  AgentContext(AgentContextArgs),
  /// Serve read-only Model Context Protocol tools.
  Mcp,
  /// Configure a local MCP client integration.
  McpSetup,
  /// Evaluate retrieval quality against benchmark questions.
  Evaluate,
  /// Summarize progress from long-running operations.
  Progress(ProgressArgs),
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
      Self::Extract(_) => "extract",
      Self::Parse(_) => "parse",
      Self::Variables(_) => "variables",
      Self::Qa(_) => "qa",
      Self::Index(_) => "index",
      Self::Search(_) => "search",
      Self::AgentContext(_) => "agent-context",
      Self::Mcp => "mcp",
      Self::McpSetup => "mcp-setup",
      Self::Evaluate => "evaluate",
      Self::Progress(_) => "progress",
      Self::Integration => "integration",
    }
  }
}
