//! Typed command-line parsing for the single `rkb` executable.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// Bounded retry example shown in archive workspace summaries when rate limits occur.
pub const ARCHIVE_RETRY_COMMAND_EXAMPLE: &str = "rkb archive --retry-failed-only --max-downloads 50 --request-delay-seconds 5 --rate-limit-cooldown-seconds 300";

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
  #[arg(
    long,
    default_value = "https://resdac.org/cms-data",
    help = "ResDAC listing root URL to crawl"
  )]
  pub base_url: String,

  #[arg(
    long,
    default_value_t = 4,
    aliases = ["max-listing-pages"],
    help = "Maximum ResDAC listing pages to crawl. Follow-up pages are controlled separately"
  )]
  pub max_pages: usize,

  #[arg(
    long,
    help = "Maximum discovered dataset/documentation pages to fetch after listing pages"
  )]
  pub max_follow_pages: Option<usize>,

  #[arg(long, help = "Maximum unique asset URLs to inventory and probe")]
  pub max_assets: Option<usize>,

  #[arg(
    long,
    default_value = "manifests/site_inventory.csv",
    help = "Inventory CSV output path"
  )]
  pub output: PathBuf,

  #[arg(
    long,
    default_value = "manifests/site_inventory_edges.csv",
    help = "Provenance edge CSV output path"
  )]
  pub edge_output: PathBuf,

  #[arg(
    long,
    default_value = "_workspace",
    help = "Workspace directory for summaries and logs"
  )]
  pub workspace_dir: PathBuf,

  #[arg(
    long,
    default_value = "_workspace/02_inventory_progress.jsonl",
    help = "JSONL progress log path"
  )]
  pub progress_log: PathBuf,

  #[arg(long, help = "Disable JSONL progress logging")]
  pub no_progress_log: bool,

  #[arg(long, default_value_t = 20.0, help = "HTTP request timeout in seconds")]
  pub timeout_seconds: f64,

  #[arg(
    long,
    default_value_t = 0.5,
    help = "Delay between HTTP requests in seconds"
  )]
  pub request_delay_seconds: f64,

  #[arg(
    long,
    default_value_t = 25,
    help = "Emit rollup progress after this many processed rows; use 0 to disable"
  )]
  pub progress_interval: usize,
}

/// Arguments for the `archive` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct ArchiveArgs {
  #[arg(
    long,
    default_value = "manifests/site_inventory.csv",
    help = "Site inventory CSV input path"
  )]
  pub inventory: PathBuf,

  #[arg(
    long,
    default_value = "data/raw",
    help = "Root directory for archived raw files"
  )]
  pub raw_root: PathBuf,

  #[arg(
    long,
    default_value = "manifests/archive_manifest.csv",
    help = "Archive manifest CSV output path"
  )]
  pub manifest_output: PathBuf,

  #[arg(
    long,
    default_value = "_workspace",
    help = "Workspace directory for summaries and logs"
  )]
  pub workspace_dir: PathBuf,

  #[arg(long, default_value_t = 20.0, help = "HTTP request timeout in seconds")]
  pub timeout_seconds: f64,

  #[arg(
    long,
    default_value_t = 0.5,
    help = "Delay between HTTP requests in seconds"
  )]
  pub request_delay_seconds: f64,

  #[arg(
    long,
    default_value_t = 5,
    help = "Defer remaining variable pages after this many consecutive HTTP 429 responses"
  )]
  pub max_consecutive_rate_limits: usize,

  #[arg(
    long,
    help = "Only retry rows that failed or were deferred in the previous archive manifest"
  )]
  pub retry_failed_only: bool,

  #[arg(
    long,
    help = "Maximum fresh network download attempts for this archive run"
  )]
  pub max_downloads: Option<usize>,

  #[arg(
    long,
    default_value_t = 0.0,
    help = "Additional cooldown after a final HTTP 429 response"
  )]
  pub rate_limit_cooldown_seconds: f64,

  #[arg(
    long,
    default_value = "_workspace/03_archive_progress.jsonl",
    help = "JSONL progress log path"
  )]
  pub progress_log: PathBuf,

  #[arg(long, help = "Disable JSONL progress logging")]
  pub no_progress_log: bool,

  #[arg(
    long,
    default_value_t = 25,
    help = "Emit rollup progress after this many processed inventory rows; use 0 to disable"
  )]
  pub progress_interval: usize,
}

/// Arguments for the `extract` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct ExtractArgs {
  #[arg(
    long,
    default_value = "manifests/archive_manifest.csv",
    help = "Archive manifest CSV input path"
  )]
  pub archive_manifest: PathBuf,

  #[arg(
    long,
    default_value = "data/metadata",
    help = "Metadata output directory"
  )]
  pub metadata_dir: PathBuf,

  #[arg(
    long,
    default_value = "data/graph",
    help = "Graph artifact output directory"
  )]
  pub graph_dir: PathBuf,

  #[arg(
    long,
    default_value = "_workspace",
    help = "Workspace directory for summaries"
  )]
  pub workspace_dir: PathBuf,
}

/// Arguments for the `parse` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct ParseArgs {
  #[arg(
    long,
    default_value = "data/metadata/datasets.csv",
    help = "Datasets metadata CSV input path"
  )]
  pub datasets_metadata: PathBuf,

  #[arg(
    long,
    default_value = "data/metadata/documents.csv",
    help = "Documents metadata CSV input path"
  )]
  pub documents_metadata: PathBuf,

  #[arg(
    long,
    default_value = "data/parsed",
    help = "Parsed text and chunk output root"
  )]
  pub parsed_root: PathBuf,

  #[arg(
    long,
    default_value = "_workspace",
    help = "Workspace directory for summaries"
  )]
  pub workspace_dir: PathBuf,

  #[arg(long, default_value_t = 500, help = "Maximum words per text chunk")]
  pub chunk_size: usize,

  #[arg(
    long,
    default_value_t = 100,
    help = "Word overlap between consecutive chunks"
  )]
  pub chunk_overlap: usize,
}

/// Arguments for the `variables` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct VariablesArgs {
  #[arg(
    long,
    default_value = "data/parsed/chunks.jsonl",
    help = "Parsed chunks JSONL input path"
  )]
  pub chunks_jsonl: PathBuf,

  #[arg(
    long,
    default_value = "manifests/archive_manifest.csv",
    help = "Archive manifest CSV input path"
  )]
  pub archive_manifest: PathBuf,

  #[arg(
    long,
    default_value = "data/metadata",
    help = "Variable metadata output directory"
  )]
  pub metadata_dir: PathBuf,

  #[arg(
    long,
    default_value = "data/graph",
    help = "Variable graph output directory"
  )]
  pub graph_dir: PathBuf,

  #[arg(
    long,
    default_value = "_workspace",
    help = "Workspace directory for summaries"
  )]
  pub workspace_dir: PathBuf,
}

/// Arguments for the `qa` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct QaArgs {
  #[arg(
    long,
    default_value = "data/metadata/datasets.csv",
    help = "Datasets metadata CSV path"
  )]
  pub datasets_metadata: PathBuf,
  #[arg(
    long,
    default_value = "data/metadata/documents.csv",
    help = "Documents metadata CSV path"
  )]
  pub documents_metadata: PathBuf,
  #[arg(
    long,
    default_value = "data/metadata/variables.csv",
    help = "Variables metadata CSV path"
  )]
  pub variables_metadata: PathBuf,
  #[arg(
    long,
    default_value = "data/metadata/canonical_variables.csv",
    help = "Canonical variables metadata CSV path"
  )]
  pub canonical_variables_metadata: PathBuf,
  #[arg(
    long,
    default_value = "data/graph/document_edges.csv",
    help = "Document edges CSV path"
  )]
  pub document_edges: PathBuf,
  #[arg(
    long,
    default_value = "data/graph/variable_edges.csv",
    help = "Variable edges CSV path"
  )]
  pub variable_edges: PathBuf,
  #[arg(
    long,
    default_value = "data/graph/data_source_variable_edges.csv",
    help = "Data source variable edges CSV path"
  )]
  pub data_source_variable_edges: PathBuf,
  #[arg(
    long,
    default_value = "data/graph/ontology_nodes.csv",
    help = "Ontology nodes CSV path"
  )]
  pub ontology_nodes: PathBuf,
  #[arg(
    long,
    default_value = "data/graph/ontology_edges.csv",
    help = "Ontology edges CSV path"
  )]
  pub ontology_edges: PathBuf,
  #[arg(
    long,
    default_value = "manifests/archive_manifest.csv",
    help = "Archive manifest CSV path"
  )]
  pub archive_manifest: PathBuf,
  #[arg(
    long,
    default_value = "_workspace",
    help = "Workspace directory for QA review output"
  )]
  pub workspace_dir: PathBuf,
}

/// Shared artifact paths for lexical indexing and retrieval.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct RetrievalPathsArgs {
  #[arg(
    long,
    default_value = "data/metadata/datasets.csv",
    help = "Datasets metadata CSV path"
  )]
  pub datasets_metadata: PathBuf,
  #[arg(
    long,
    default_value = "data/metadata/documents.csv",
    help = "Documents metadata CSV path"
  )]
  pub documents_metadata: PathBuf,
  #[arg(
    long,
    default_value = "data/metadata/variables.csv",
    help = "Variables metadata CSV path"
  )]
  pub variables_metadata: PathBuf,
  #[arg(
    long,
    default_value = "data/parsed/chunks.jsonl",
    help = "Parsed chunks JSONL path"
  )]
  pub chunks_jsonl: PathBuf,
  #[arg(
    long,
    default_value = "data/index/retrieval.sqlite",
    help = "SQLite retrieval index path"
  )]
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
  #[arg(
    long,
    help = "Build optional deterministic embedding table for hybrid search"
  )]
  pub build_embeddings: bool,
  #[arg(
    long,
    default_value = "all-MiniLM-L6-v2",
    help = "SentenceTransformer model name for hybrid embeddings"
  )]
  pub semantic_model_name: String,
}

/// Arguments for the `search` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct SearchArgs {
  #[arg(long, help = "Search query text")]
  pub query: String,
  #[arg(
    long,
    default_value_t = 10,
    help = "Maximum number of results to return"
  )]
  pub limit: usize,
  #[arg(long, help = "Emit JSON output")]
  pub json: bool,
  #[arg(long, help = "Enable hybrid search (semantic reranking)")]
  pub hybrid: bool,
  #[arg(long, default_value_t = 0.5, help = "Semantic blend weight (0 to 1)")]
  pub semantic_weight: f64,
  #[arg(
    long,
    default_value = "all-MiniLM-L6-v2",
    help = "SentenceTransformer model name for hybrid search"
  )]
  pub semantic_model_name: String,
  #[command(flatten)]
  pub paths: RetrievalPathsArgs,
}

/// Arguments for the `agent-context` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct AgentContextArgs {
  #[arg(long, help = "Context query text")]
  pub query: String,
  #[arg(
    long,
    default_value_t = crate::config::AgentContextConfig::DEFAULT_LIMIT,
    help = "Maximum number of retrieval results to include"
  )]
  pub limit: usize,
  #[arg(long, help = "Emit JSON output")]
  pub json: bool,
  #[arg(long, help = "Enable hybrid search (semantic reranking)")]
  pub hybrid: bool,
  #[arg(long, default_value_t = 0.5, help = "Semantic blend weight (0 to 1)")]
  pub semantic_weight: f64,
  #[arg(
    long,
    default_value = "all-MiniLM-L6-v2",
    help = "SentenceTransformer model name for hybrid search"
  )]
  pub semantic_model_name: String,
  #[command(flatten)]
  pub paths: RetrievalPathsArgs,
}

/// Arguments for the `mcp` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct McpArgs {
  #[arg(
    long,
    default_value_t = crate::config::AgentContextConfig::DEFAULT_LIMIT,
    help = "Default result limit for MCP retrieval tools"
  )]
  pub default_limit: usize,
  #[arg(
    long,
    default_value = "_workspace",
    help = "Workspace directory for MCP lifecycle state"
  )]
  pub workspace_dir: PathBuf,
  #[command(flatten)]
  pub paths: RetrievalPathsArgs,
  #[command(subcommand)]
  pub lifecycle: Option<McpLifecycleCommand>,
}

/// Arguments for the `mcp-setup` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct McpSetupArgs {
  #[arg(long = "client", help = "MCP client to configure (repeatable)")]
  pub clients: Vec<String>,
  #[arg(
    long,
    default_value = ".",
    help = "Project path containing client config files"
  )]
  pub project_path: PathBuf,
  #[arg(long, help = "Explicit client config file path")]
  pub config_path: Option<PathBuf>,
  #[arg(
    long,
    default_value = "rkb",
    help = "Server command to write into client config"
  )]
  pub command: String,
  #[arg(
    long,
    default_value = "rkb",
    help = "Server name to write into client config"
  )]
  pub server_name: String,
  #[arg(long, help = "Print planned config changes without writing files")]
  pub dry_run: bool,
  #[arg(long, help = "Overwrite existing MCP server entries")]
  pub force: bool,
}

/// Arguments for the `integration` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct IntegrationArgs {
  #[command(subcommand)]
  pub command: IntegrationCommand,
}

/// Downstream integration helper commands.
#[derive(Clone, Debug, PartialEq, Subcommand)]
pub enum IntegrationCommand {
  /// Return dataset availability years or check one year.
  Availability {
    #[arg(long, help = "Dataset identifier")]
    dataset: String,
    #[arg(long, help = "Optional year to check for availability")]
    year: Option<u16>,
    #[command(flatten)]
    paths: RetrievalPathsArgs,
  },
  /// Map variable names to dataset-specific records.
  Crosswalk {
    #[arg(long, value_delimiter = ',', help = "Variable names to crosswalk")]
    variables: Vec<String>,
    #[command(flatten)]
    paths: RetrievalPathsArgs,
  },
  /// Generate a cohort variable dictionary.
  CohortDictionary {
    #[arg(
      long,
      value_delimiter = ',',
      help = "Variable names for the cohort dictionary"
    )]
    variables: Vec<String>,
    #[command(flatten)]
    paths: RetrievalPathsArgs,
  },
  /// Format agent context as prompt, markdown, or XML.
  FormatContext {
    #[arg(long, help = "Context query text")]
    query: String,
    #[arg(
      long,
      default_value = "prompt",
      help = "Output format: prompt, markdown, or xml"
    )]
    format: String,
    #[arg(
      long,
      default_value_t = crate::config::AgentContextConfig::DEFAULT_LIMIT,
      help = "Maximum number of retrieval results to include"
    )]
    limit: usize,
    #[command(flatten)]
    paths: RetrievalPathsArgs,
  },
  /// Scan code files for dataset and variable caveats.
  ScanCaveats {
    #[arg(long = "files", value_delimiter = ',', help = "Code files to scan")]
    files: Vec<PathBuf>,
    #[arg(
      long = "keywords",
      value_delimiter = ',',
      help = "Keywords to match in code"
    )]
    keywords: Vec<String>,
    #[command(flatten)]
    paths: RetrievalPathsArgs,
  },
}

/// Background MCP lifecycle commands.
#[derive(Clone, Debug, PartialEq, Subcommand)]
pub enum McpLifecycleCommand {
  /// Record MCP background server startup state.
  Start {
    #[arg(
      long,
      default_value = "127.0.0.1",
      help = "Host address for lifecycle state"
    )]
    host: String,
    #[arg(long, default_value_t = 8000, help = "Port for lifecycle state")]
    port: u16,
  },
  /// Show recorded MCP background server status.
  Status,
  /// Stop the recorded MCP background server.
  Stop,
}

/// Arguments for the `progress` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct ProgressArgs {
  #[arg(
    long = "log",
    help = "Progress JSONL log path (repeatable; defaults to inventory and archive logs)"
  )]
  pub logs: Vec<PathBuf>,
  #[arg(long, help = "Emit JSON output")]
  pub json: bool,
}

/// Arguments for the `evaluate` subcommand.
#[derive(Args, Clone, Debug, PartialEq)]
pub struct EvaluateArgs {
  #[arg(
    long,
    default_value_t = crate::config::VariableEvaluationConfig::DEFAULT_SAMPLE_SIZE,
    help = "Number of variables to sample for retrieval evaluation"
  )]
  pub sample_size: usize,
  #[arg(
    long,
    default_value_t = crate::config::VariableEvaluationConfig::DEFAULT_SEED,
    help = "Random seed for deterministic variable sampling"
  )]
  pub seed: u64,
  #[arg(
    long,
    default_value_t = crate::config::VariableEvaluationConfig::DEFAULT_LIMIT,
    help = "Maximum retrieval results per evaluated variable"
  )]
  pub limit: usize,
  #[arg(long, help = "Emit JSON output")]
  pub json: bool,
  #[arg(
    long,
    num_args = 0..=1,
    default_missing_value = "data/evaluation/benchmark_questions.json",
    help = "Optional benchmark questions JSON path"
  )]
  pub benchmark: Option<PathBuf>,
  #[arg(
    long,
    default_value = "_workspace/retrieval_evaluation_report.md",
    help = "Benchmark evaluation Markdown report output path"
  )]
  pub output_report: PathBuf,
  #[arg(
    long,
    default_value = "manifests/archive_manifest.csv",
    help = "Archive manifest CSV path for evaluation context"
  )]
  pub archive_manifest_path: PathBuf,
  #[command(flatten)]
  pub paths: RetrievalPathsArgs,
}

impl EvaluateArgs {
  #[must_use]
  pub fn into_config(&self) -> crate::config::VariableEvaluationConfig {
    crate::config::VariableEvaluationConfig {
      retrieval: self.paths.clone().into_config(),
      archive_manifest_path: self.archive_manifest_path.clone(),
      sample_size: self.sample_size,
      seed: self.seed,
      limit: self.limit,
    }
  }
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
  Mcp(McpArgs),
  /// Configure a local MCP client integration.
  McpSetup(McpSetupArgs),
  /// Evaluate retrieval quality against benchmark questions.
  Evaluate(EvaluateArgs),
  /// Summarize progress from long-running operations.
  Progress(ProgressArgs),
  /// Run downstream integration helpers.
  Integration(IntegrationArgs),
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
      Self::Mcp(_) => "mcp",
      Self::McpSetup(_) => "mcp-setup",
      Self::Evaluate(_) => "evaluate",
      Self::Progress(_) => "progress",
      Self::Integration(_) => "integration",
    }
  }
}
