//! Shared contracts for the `rkb` command-line program.
//!
//! The foundation release reserves the CLI namespace without claiming that
//! pipeline behavior has been ported. Each command will replace its explicit
//! unavailable result with a tested implementation in a later rewrite slice.

pub mod archive;
pub mod cli;
pub mod config;
pub mod error;
pub mod extract;
pub mod inventory;
pub mod parse;
pub mod paths;
pub mod progress;
pub mod records;
pub mod variables;

use cli::Command;
use config::{ArchiveConfig, InventoryConfig};
pub use error::AppError;

/// Dispatches one parsed command.
///
/// # Errors
///
/// Returns [`AppError`] or [`AppError::CommandUnavailable`] until the
/// selected command has a verified Rust implementation.
#[allow(clippy::too_many_lines)]
pub fn run(command: Command) -> Result<(), AppError> {
  match command {
    Command::Inventory(args) => {
      let mut config = InventoryConfig {
        base_url: args.base_url,
        max_pages: args.max_pages,
        max_follow_pages: args.max_follow_pages,
        max_assets: args.max_assets,
        timeout_seconds: args.timeout_seconds,
        request_delay_seconds: args.request_delay_seconds,
        progress_interval: args.progress_interval,
        progress_log_path: if args.no_progress_log {
          None
        } else {
          Some(args.progress_log)
        },
        user_agent: "Mozilla/5.0 (compatible; cms-kb-inventory/0.1)".to_string(),
        output_path: args.output,
        edge_output_path: args.edge_output,
        workspace_dir: args.workspace_dir,
      };
      config.validate()?;
      inventory::run_inventory(&config)?;
      Ok(())
    }
    Command::Archive(args) => {
      let config = ArchiveConfig {
        inventory_path: args.inventory,
        raw_root: args.raw_root,
        manifest_output_path: args.manifest_output,
        workspace_dir: args.workspace_dir,
        timeout_seconds: args.timeout_seconds,
        request_delay_seconds: args.request_delay_seconds,
        max_consecutive_rate_limits: args.max_consecutive_rate_limits,
        retry_failed_only: args.retry_failed_only,
        max_downloads: args.max_downloads,
        rate_limit_cooldown_seconds: args.rate_limit_cooldown_seconds,
        progress_log_path: if args.no_progress_log {
          None
        } else {
          Some(args.progress_log)
        },
        progress_interval: args.progress_interval,
        user_agent: "Mozilla/5.0 (compatible; cms-kb-archive/0.1)".to_string(),
      };
      config.validate()?;
      archive::run_archive_default(&config)?;
      Ok(())
    }
    Command::Extract(args) => {
      let config = config::ExtractionConfig {
        archive_manifest_path: args.archive_manifest,
        metadata_dir: args.metadata_dir,
        graph_dir: args.graph_dir,
        workspace_dir: args.workspace_dir,
      };
      extract::run_extraction(&config)?;
      Ok(())
    }
    Command::Parse(args) => {
      let config = config::ParsingConfig {
        datasets_metadata_path: args.datasets_metadata,
        documents_metadata_path: args.documents_metadata,
        parsed_root: args.parsed_root,
        workspace_dir: args.workspace_dir,
        chunk_size: args.chunk_size,
        chunk_overlap: args.chunk_overlap,
      };
      config.validate()?;
      parse::run_parsing(&config)?;
      Ok(())
    }
    Command::Variables(args) => {
      let config = config::VariableExtractionConfig {
        chunks_jsonl_path: args.chunks_jsonl,
        archive_manifest_path: args.archive_manifest,
        metadata_dir: args.metadata_dir,
        graph_dir: args.graph_dir,
        workspace_dir: args.workspace_dir,
      };
      let result = variables::run_variable_extraction(&config)?;
      println!(
        "wrote {} variables and {} variable edges; wrote {} canonical variables and {} data source variable edges; summary: {}",
        result.variables.len(),
        result.edges.len(),
        result.canonical_variables.len(),
        result.data_source_variable_edges.len(),
        result.summary_path.display()
      );
      if result.failures.is_empty() {
        Ok(())
      } else {
        Err(AppError::RecordParseError(format!(
          "variable extraction completed with {} failures; see workspace summary pack",
          result.failures.len()
        )))
      }
    }
    _ => Err(AppError::CommandUnavailable {
      command: command.name(),
    }),
  }
}
