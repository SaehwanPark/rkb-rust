//! Typed configuration boundaries for the knowledge base pipeline.

use crate::error::AppError;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct InventoryConfig {
  pub base_url: String,
  pub max_pages: usize,
  pub max_follow_pages: Option<usize>,
  pub max_assets: Option<usize>,
  pub timeout_seconds: f64,
  pub request_delay_seconds: f64,
  pub progress_interval: usize,
  pub progress_log_path: Option<PathBuf>,
  pub user_agent: String,
  pub output_path: PathBuf,
  pub edge_output_path: PathBuf,
  pub workspace_dir: PathBuf,
}

impl Default for InventoryConfig {
  fn default() -> Self {
    Self {
      base_url: "https://resdac.org/cms-data".to_string(),
      max_pages: 4,
      max_follow_pages: None,
      max_assets: None,
      timeout_seconds: 20.0,
      request_delay_seconds: 0.0,
      progress_interval: 25,
      progress_log_path: None,
      user_agent: "Mozilla/5.0 (compatible; cms-kb-inventory/0.1)".to_string(),
      output_path: PathBuf::from("manifests/site_inventory.csv"),
      edge_output_path: PathBuf::from("manifests/site_inventory_edges.csv"),
      workspace_dir: PathBuf::from("_workspace"),
    }
  }
}

impl InventoryConfig {
  /// Validates configuration constraints.
  ///
  /// # Errors
  ///
  /// Returns `AppError::ConfigValidationError` if validation fails.
  pub fn validate(&mut self) -> Result<(), AppError> {
    // Validate base_url
    if !(self.base_url.starts_with("http://") || self.base_url.starts_with("https://")) {
      return Err(AppError::ConfigValidationError(
        "base_url must start with http:// or https://".to_string(),
      ));
    }
    let scheme_len = if self.base_url.starts_with("https://") {
      8
    } else {
      7
    };
    let rest = &self.base_url[scheme_len..];
    let host = rest.split('/').next().unwrap_or("");
    if host.is_empty() {
      return Err(AppError::ConfigValidationError(
        "base_url must contain a non-empty host name".to_string(),
      ));
    }
    // Strip trailing slash
    self.base_url = self.base_url.trim_end_matches('/').to_string();

    // Validate page bounds
    if self.max_pages < 1 {
      return Err(AppError::ConfigValidationError(
        "max_pages must be at least 1".to_string(),
      ));
    }

    // Validate timeouts and delays
    if !self.timeout_seconds.is_finite() || self.timeout_seconds <= 0.0 {
      return Err(AppError::ConfigValidationError(
        "timeout_seconds must be a finite number greater than 0".to_string(),
      ));
    }
    if !self.request_delay_seconds.is_finite() || self.request_delay_seconds < 0.0 {
      return Err(AppError::ConfigValidationError(
        "request_delay_seconds cannot be negative".to_string(),
      ));
    }

    Ok(())
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArchiveConfig {
  pub inventory_path: PathBuf,
  pub raw_root: PathBuf,
  pub manifest_output_path: PathBuf,
  pub workspace_dir: PathBuf,
  pub timeout_seconds: f64,
  pub request_delay_seconds: f64,
  pub max_consecutive_rate_limits: usize,
  pub retry_failed_only: bool,
  pub max_downloads: Option<usize>,
  pub rate_limit_cooldown_seconds: f64,
  pub progress_log_path: Option<PathBuf>,
  pub progress_interval: usize,
  pub user_agent: String,
}

impl Default for ArchiveConfig {
  fn default() -> Self {
    Self {
      inventory_path: PathBuf::from("manifests/site_inventory.csv"),
      raw_root: PathBuf::from("data/raw"),
      manifest_output_path: PathBuf::from("manifests/archive_manifest.csv"),
      workspace_dir: PathBuf::from("_workspace"),
      timeout_seconds: 20.0,
      request_delay_seconds: 0.0,
      max_consecutive_rate_limits: 5,
      retry_failed_only: false,
      max_downloads: None,
      rate_limit_cooldown_seconds: 0.0,
      progress_log_path: None,
      progress_interval: 25,
      user_agent: "Mozilla/5.0 (compatible; cms-kb-archive/0.1)".to_string(),
    }
  }
}

impl ArchiveConfig {
  /// Validates configuration constraints.
  ///
  /// # Errors
  ///
  /// Returns `AppError::ConfigValidationError` if validation fails.
  pub fn validate(&self) -> Result<(), AppError> {
    if !self.timeout_seconds.is_finite() || self.timeout_seconds <= 0.0 {
      return Err(AppError::ConfigValidationError(
        "timeout_seconds must be a finite number greater than 0".to_string(),
      ));
    }
    if !self.request_delay_seconds.is_finite() || self.request_delay_seconds < 0.0 {
      return Err(AppError::ConfigValidationError(
        "request_delay_seconds must be greater than or equal to 0".to_string(),
      ));
    }
    if self.max_consecutive_rate_limits < 1 {
      return Err(AppError::ConfigValidationError(
        "max_consecutive_rate_limits must be greater than 0".to_string(),
      ));
    }
    if !self.rate_limit_cooldown_seconds.is_finite() || self.rate_limit_cooldown_seconds < 0.0 {
      return Err(AppError::ConfigValidationError(
        "rate_limit_cooldown_seconds must be greater than or equal to 0".to_string(),
      ));
    }
    Ok(())
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExtractionConfig {
  pub archive_manifest_path: PathBuf,
  pub metadata_dir: PathBuf,
  pub graph_dir: PathBuf,
  pub workspace_dir: PathBuf,
}

impl Default for ExtractionConfig {
  fn default() -> Self {
    Self {
      archive_manifest_path: PathBuf::from("manifests/archive_manifest.csv"),
      metadata_dir: PathBuf::from("data/metadata"),
      graph_dir: PathBuf::from("data/graph"),
      workspace_dir: PathBuf::from("_workspace"),
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParsingConfig {
  pub datasets_metadata_path: PathBuf,
  pub documents_metadata_path: PathBuf,
  pub parsed_root: PathBuf,
  pub workspace_dir: PathBuf,
  pub chunk_size: usize,
  pub chunk_overlap: usize,
}

impl Default for ParsingConfig {
  fn default() -> Self {
    Self {
      datasets_metadata_path: crate::paths::get_packaged_data_path("metadata/datasets.csv"),
      documents_metadata_path: crate::paths::get_packaged_data_path("metadata/documents.csv"),
      parsed_root: crate::paths::get_packaged_data_path("parsed"),
      workspace_dir: PathBuf::from("_workspace"),
      chunk_size: 500,
      chunk_overlap: 100,
    }
  }
}

impl ParsingConfig {
  /// Validates configuration constraints.
  ///
  /// # Errors
  ///
  /// Returns `AppError::ConfigValidationError` if validation fails.
  pub fn validate(&self) -> Result<(), AppError> {
    if self.chunk_size == 0 {
      return Err(AppError::ConfigValidationError(
        "chunk_size must be greater than 0".to_string(),
      ));
    }
    if self.chunk_overlap >= self.chunk_size {
      return Err(AppError::ConfigValidationError(
        "chunk_overlap must be less than chunk_size".to_string(),
      ));
    }
    Ok(())
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VariableExtractionConfig {
  pub chunks_jsonl_path: PathBuf,
  pub archive_manifest_path: PathBuf,
  pub metadata_dir: PathBuf,
  pub graph_dir: PathBuf,
  pub workspace_dir: PathBuf,
}

impl Default for VariableExtractionConfig {
  fn default() -> Self {
    Self {
      chunks_jsonl_path: crate::paths::get_packaged_data_path("parsed/chunks.jsonl"),
      archive_manifest_path: PathBuf::from("manifests/archive_manifest.csv"),
      metadata_dir: PathBuf::from("data/metadata"),
      graph_dir: PathBuf::from("data/graph"),
      workspace_dir: PathBuf::from("_workspace"),
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct QAConfig {
  pub datasets_metadata_path: PathBuf,
  pub documents_metadata_path: PathBuf,
  pub variables_metadata_path: PathBuf,
  pub canonical_variables_metadata_path: PathBuf,
  pub document_edges_path: PathBuf,
  pub variable_edges_path: PathBuf,
  pub data_source_variable_edges_path: PathBuf,
  pub ontology_nodes_path: PathBuf,
  pub ontology_edges_path: PathBuf,
  pub archive_manifest_path: PathBuf,
  pub workspace_dir: PathBuf,
}

impl Default for QAConfig {
  fn default() -> Self {
    Self {
      datasets_metadata_path: crate::paths::get_packaged_data_path("metadata/datasets.csv"),
      documents_metadata_path: crate::paths::get_packaged_data_path("metadata/documents.csv"),
      variables_metadata_path: crate::paths::get_packaged_data_path("metadata/variables.csv"),
      canonical_variables_metadata_path: crate::paths::get_packaged_data_path(
        "metadata/canonical_variables.csv",
      ),
      document_edges_path: crate::paths::get_packaged_data_path("graph/document_edges.csv"),
      variable_edges_path: crate::paths::get_packaged_data_path("graph/variable_edges.csv"),
      data_source_variable_edges_path: crate::paths::get_packaged_data_path(
        "graph/data_source_variable_edges.csv",
      ),
      ontology_nodes_path: crate::paths::get_packaged_data_path("graph/ontology_nodes.csv"),
      ontology_edges_path: crate::paths::get_packaged_data_path("graph/ontology_edges.csv"),
      archive_manifest_path: PathBuf::from("manifests/archive_manifest.csv"),
      workspace_dir: PathBuf::from("_workspace"),
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RetrievalConfig {
  pub datasets_metadata_path: PathBuf,
  pub documents_metadata_path: PathBuf,
  pub variables_metadata_path: PathBuf,
  pub chunks_jsonl_path: PathBuf,
  pub database_path: PathBuf,
  pub hybrid_search_enabled: bool,
  pub semantic_model_name: String,
  pub semantic_weight: f64,
}

impl Default for RetrievalConfig {
  fn default() -> Self {
    Self {
      datasets_metadata_path: crate::paths::get_packaged_data_path("metadata/datasets.csv"),
      documents_metadata_path: crate::paths::get_packaged_data_path("metadata/documents.csv"),
      variables_metadata_path: crate::paths::get_packaged_data_path("metadata/variables.csv"),
      chunks_jsonl_path: crate::paths::get_packaged_data_path("parsed/chunks.jsonl"),
      database_path: crate::paths::get_packaged_data_path("index/retrieval.sqlite"),
      hybrid_search_enabled: false,
      semantic_model_name: "all-MiniLM-L6-v2".to_string(),
      semantic_weight: 0.5,
    }
  }
}

impl RetrievalConfig {
  /// Validates configuration constraints.
  ///
  /// # Errors
  ///
  /// Returns `AppError::ConfigValidationError` if validation fails.
  pub fn validate(&self) -> Result<(), AppError> {
    if !self.semantic_weight.is_finite() || !(0.0..=1.0).contains(&self.semantic_weight) {
      return Err(AppError::ConfigValidationError(
        "semantic_weight must be a finite number between 0.0 and 1.0 inclusive".to_string(),
      ));
    }
    Ok(())
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentContextConfig {
  pub retrieval: RetrievalConfig,
  pub archive_manifest_path: PathBuf,
  pub default_limit: usize,
}

impl Default for AgentContextConfig {
  fn default() -> Self {
    Self {
      retrieval: RetrievalConfig::default(),
      archive_manifest_path: PathBuf::from("manifests/archive_manifest.csv"),
      default_limit: Self::DEFAULT_LIMIT,
    }
  }
}

impl AgentContextConfig {
  pub const DEFAULT_LIMIT: usize = 5;

  /// Validates configuration constraints.
  ///
  /// # Errors
  ///
  /// Returns `AppError::ConfigValidationError` if validation fails.
  pub fn validate(&self) -> Result<(), AppError> {
    self.retrieval.validate()
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VariableEvaluationConfig {
  pub retrieval: RetrievalConfig,
  pub archive_manifest_path: PathBuf,
  pub sample_size: usize,
  pub seed: u64,
  pub limit: usize,
}

impl Default for VariableEvaluationConfig {
  fn default() -> Self {
    Self {
      retrieval: RetrievalConfig::default(),
      archive_manifest_path: PathBuf::from("manifests/archive_manifest.csv"),
      sample_size: 10,
      seed: 20_260_616,
      limit: 5,
    }
  }
}

impl VariableEvaluationConfig {
  pub const DEFAULT_SAMPLE_SIZE: usize = 10;
  pub const DEFAULT_SEED: u64 = 20_260_616;
  pub const DEFAULT_LIMIT: usize = 5;

  /// Validates configuration constraints.
  ///
  /// # Errors
  ///
  /// Returns `AppError::ConfigValidationError` if validation fails.
  pub fn validate(&self) -> Result<(), AppError> {
    if self.sample_size == 0 {
      return Err(AppError::ConfigValidationError(
        "sample_size must be greater than 0".to_string(),
      ));
    }
    if self.limit == 0 {
      return Err(AppError::ConfigValidationError(
        "limit must be greater than 0".to_string(),
      ));
    }
    self.retrieval.validate()
  }
}
