//! Shared contracts for the `rkb` command-line program.
//!
//! Implemented subcommands replace reserved namespace entries one verified
//! rewrite slice at a time.

pub mod agent_context;
pub mod archive;
pub mod cli;
pub mod config;
pub mod error;
pub mod evaluation;
pub mod extract;
pub mod integration;
pub mod inventory;
pub mod mcp;
pub mod mcp_setup;
pub mod parse;
pub mod paths;
pub mod progress;
pub mod qa;
pub mod records;
pub mod retrieval;
pub mod variables;

use cli::Command;
use cli::IntegrationCommand;
use cli::McpLifecycleCommand;
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
      let mut config = args.paths.into_config();
      config.semantic_model_name = args.semantic_model_name;
      println!(
        "Building search index at {}...",
        config.database_path.display()
      );
      retrieval::build_index_with_options(&config, args.build_embeddings)?;
      println!("Search index built successfully.");
      Ok(())
    }
    Command::Search(args) => {
      let mut config = args.paths.into_config();
      config.hybrid_search_enabled = args.hybrid;
      config.semantic_weight = args.semantic_weight;
      config.semantic_model_name = args.semantic_model_name;
      let results = retrieval::run_retrieval(&config, &args.query, args.limit)?;
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
      let mut config = args.paths.into_config();
      config.hybrid_search_enabled = args.hybrid;
      config.semantic_weight = args.semantic_weight;
      config.semantic_model_name = args.semantic_model_name;
      let results = retrieval::run_retrieval(&config, &args.query, args.limit)?;
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
    Command::Mcp(args) => match args.lifecycle {
      Some(McpLifecycleCommand::Start { host, port }) => {
        let state = mcp::start_background_state(&args.workspace_dir, &host, port)?;
        println!("Recording MCP server background state");
        println!(
          "MCP server state recorded successfully: PID {} at {}",
          state.pid, state.endpoint_url
        );
        Ok(())
      }
      Some(McpLifecycleCommand::Status) => {
        match mcp::read_background_state(&args.workspace_dir)? {
          Some(state) => {
            println!("MCP server status: recorded");
            println!("PID: {}", state.pid);
            println!("Host: {}", state.host);
            println!("Port: {}", state.port);
            println!("Endpoint: {}", state.endpoint_url);
            println!("Log: {}", state.log_path.display());
          }
          None => println!("MCP server status: stopped"),
        }
        Ok(())
      }
      Some(McpLifecycleCommand::Stop) => {
        let state = mcp::stop_background_state(&args.workspace_dir)?;
        println!("Stopping MCP server (PID: {})", state.pid);
        println!("MCP server stopped successfully.");
        Ok(())
      }
      None => {
        let config = mcp::McpConfig {
          retrieval: args.paths.into_config(),
          default_limit: args.default_limit,
        };
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        mcp::run_stdio_server(&config, stdin.lock(), stdout.lock())
      }
    },
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
    Command::Evaluate(args) => {
      let config = args.into_config();
      if let Some(benchmark_path) = &args.benchmark {
        let suite = evaluation::read_benchmark_suite(benchmark_path)?;
        let report = evaluation::evaluate_benchmark_suite(&config, &suite)?;
        evaluation::generate_markdown_report(&report, &args.output_report)?;
        if args.json {
          println!(
            "{}",
            serde_json::to_string_pretty(&report)
              .map_err(|error| AppError::RetrievalError(error.to_string()))?
          );
        } else {
          println!(
            "{}",
            evaluation::format_benchmark_report_text(&report, &args.output_report)
          );
        }
        Ok(())
      } else {
        let report = evaluation::evaluate_variable_retrieval(&config)?;
        if args.json {
          println!("{}", evaluation::variable_report_to_json(&report)?);
        } else {
          println!("{}", evaluation::format_variable_report_text(&report));
        }
        if report.passed_count() == report.sample_size {
          Ok(())
        } else {
          Err(AppError::RetrievalError(format!(
            "variable retrieval evaluation failed: {}/{} passed",
            report.passed_count(),
            report.sample_size
          )))
        }
      }
    }
    Command::McpSetup(args) => {
      if args.clients.is_empty() {
        return Err(AppError::ConfigValidationError(
          "at least one --client is required for non-interactive mcp-setup".to_string(),
        ));
      }
      for client in &args.clients {
        let message = mcp_setup::configure_client(
          client,
          args.config_path.as_deref(),
          &args.project_path,
          &args.server_name,
          &args.command,
          args.dry_run,
        )?;
        println!("{message}");
      }
      Ok(())
    }
    Command::Integration(args) => {
      match args.command {
        IntegrationCommand::Availability {
          dataset,
          year,
          paths,
        } => {
          let availability = integration::dataset_availability(&paths.into_config(), &dataset)?;
          if let Some(year) = year {
            println!("{}", availability.available_years.contains(&year));
          } else {
            println!(
              "{}",
              serde_json::to_string_pretty(&availability)
                .map_err(|error| AppError::RecordParseError(error.to_string()))?
            );
          }
        }
        IntegrationCommand::Crosswalk { variables, paths } => {
          let variables = variables
            .into_iter()
            .map(|variable| variable.trim().to_string())
            .filter(|variable| !variable.is_empty())
            .collect::<Vec<_>>();
          let response = integration::crosswalk_variables(&paths.into_config(), &variables)?;
          println!(
            "{}",
            serde_json::to_string_pretty(&response)
              .map_err(|error| AppError::RecordParseError(error.to_string()))?
          );
        }
        IntegrationCommand::CohortDictionary { variables, paths } => {
          let variables = variables
            .into_iter()
            .map(|variable| variable.trim().to_string())
            .filter(|variable| !variable.is_empty())
            .collect::<Vec<_>>();
          let response = integration::cohort_dictionary(&paths.into_config(), &variables)?;
          println!(
            "{}",
            serde_json::to_string_pretty(&response)
              .map_err(|error| AppError::RecordParseError(error.to_string()))?
          );
        }
        IntegrationCommand::FormatContext {
          query,
          format,
          limit,
          paths,
        } => {
          println!(
            "{}",
            integration::run_format_context(&paths.into_config(), &query, limit, &format)?
          );
        }
        IntegrationCommand::ScanCaveats {
          files,
          keywords,
          paths,
        } => {
          let response =
            integration::scan_codebase_caveats(&paths.into_config(), &files, &keywords)?;
          println!(
            "{}",
            serde_json::to_string_pretty(&response)
              .map_err(|error| AppError::RecordParseError(error.to_string()))?
          );
        }
      }
      Ok(())
    }
  }
}
