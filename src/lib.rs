//! Shared contracts for the `rkb` command-line program.
//!
//! Implemented subcommands replace reserved namespace entries one verified
//! rewrite slice at a time.

pub mod agent_context;
pub mod archive;
pub mod cli;
pub mod config;
pub mod error;
pub mod extract;
pub mod inventory;
pub mod parse;
pub mod paths;
pub mod progress;
pub mod qa;
pub mod records;
pub mod retrieval;
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
    Command::Qa(args) => {
      let result = qa::run_qa(&args.into_config())?;
      let message = format!(
        "QA review finished with verdict: {}; {} errors, {} warnings; summary written to {}",
        result.verdict,
        result.error_count(),
        result.warning_count(),
        result.summary_path.display()
      );
      println!("{message}");
      if result.verdict == qa::QaVerdict::Pass {
        Ok(())
      } else {
        Err(AppError::RecordParseError(message))
      }
    }
    Command::Index(args) => {
      let config = args.paths.into_config();
      println!(
        "Building search index at {}...",
        config.database_path.display()
      );
      retrieval::build_index(&config)?;
      println!("Search index built successfully.");
      Ok(())
    }
    Command::Search(args) => {
      let results = retrieval::run_retrieval(&args.paths.into_config(), &args.query, args.limit)?;
      if args.json {
        println!(
          "{}",
          serde_json::to_string_pretty(&results)
            .map_err(|error| AppError::RetrievalError(error.to_string()))?
        );
      } else {
        for result in results {
          let page = result
            .page
            .map_or_else(String::new, |page| format!(" page {page}"));
          println!(
            "{:.3}\t{}\t{}\t{}{}\n{}",
            result.score,
            result.record_type.as_str(),
            result.record_id,
            result.source_url,
            page,
            result.snippet
          );
        }
      }
      Ok(())
    }
    Command::AgentContext(args) => {
      let results = retrieval::run_retrieval(&args.paths.into_config(), &args.query, args.limit)?;
      let context = agent_context::build_agent_context(&args.query, results);
      if args.json {
        println!(
          "{}",
          serde_json::to_string_pretty(&context)
            .map_err(|error| AppError::RetrievalError(error.to_string()))?
        );
      } else {
        println!("{}", agent_context::format_agent_context_text(&context));
      }
      Ok(())
    }
    Command::Progress(args) => {
      let explicit = !args.logs.is_empty();
      let paths = if explicit {
        args.logs
      } else {
        progress::default_progress_log_paths()
      };
      let summary = progress::summarize_progress_logs(&paths, explicit)?;
      if args.json {
        println!(
          "{}",
          serde_json::to_string_pretty(&summary)
            .map_err(|error| AppError::RecordParseError(error.to_string()))?
        );
      } else {
        println!("{}", progress::format_progress_summary_text(&summary));
      }
      Ok(())
    }
    _ => Err(AppError::CommandUnavailable {
      command: command.name(),
    }),
  }
}
