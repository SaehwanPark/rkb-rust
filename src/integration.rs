//! Downstream research integration helpers.

use crate::agent_context::{AgentContext, format_agent_context_text};
use crate::config::RetrievalConfig;
use crate::error::AppError;
use crate::retrieval::{self, RecordType};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn integration_error(message: impl Into<String>) -> AppError {
  AppError::RecordParseError(message.into())
}

fn read_csv_rows(path: &Path) -> Result<Vec<HashMap<String, String>>, AppError> {
  let mut reader = csv::Reader::from_path(path)
    .map_err(|error| integration_error(format!("failed to read {}: {error}", path.display())))?;
  let headers = reader
    .headers()
    .map_err(|error| {
      integration_error(format!("failed to read {} header: {error}", path.display()))
    })?
    .clone();
  reader
    .records()
    .map(|record| {
      let record = record.map_err(|error| {
        integration_error(format!("failed to parse {}: {error}", path.display()))
      })?;
      Ok(
        headers
          .iter()
          .zip(record.iter())
          .map(|(key, value)| (key.to_string(), value.to_string()))
          .collect(),
      )
    })
    .collect()
}

fn value(row: &HashMap<String, String>, field: &str) -> String {
  row.get(field).cloned().unwrap_or_default()
}

/// Dataset availability response.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DatasetAvailability {
  pub dataset_id: String,
  pub name: String,
  pub available_years: Vec<u16>,
}

/// Variable crosswalk item.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VariableCrosswalkItem {
  pub variable_name: String,
  pub record_id: String,
  pub dataset_id: String,
  pub dataset_name: String,
  pub definition: String,
  pub available_years: Vec<u16>,
  pub source_url: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VariableCrosswalkResponse {
  pub variables: BTreeMap<String, Vec<VariableCrosswalkItem>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CaveatScanResponse {
  pub matches: BTreeMap<String, Vec<VariableCrosswalkItem>>,
}

/// Parses all four-digit years and ranges from an availability string.
#[must_use]
pub fn parse_availability_years(input: &str) -> Vec<u16> {
  let years = input
    .as_bytes()
    .windows(4)
    .filter_map(|window| std::str::from_utf8(window).ok())
    .filter_map(|candidate| candidate.parse::<u16>().ok())
    .filter(|year| (1900..=2100).contains(year))
    .collect::<Vec<_>>();
  let mut expanded = BTreeSet::new();
  let mut index = 0;
  while index < years.len() {
    if let Some(next) = years.get(index + 1)
      && *next >= years[index]
      && *next - years[index] <= 80
    {
      for year in years[index]..=*next {
        expanded.insert(year);
      }
      index += 2;
      continue;
    }
    expanded.insert(years[index]);
    index += 1;
  }
  expanded.into_iter().collect()
}

fn datasets_by_id(
  config: &RetrievalConfig,
) -> Result<BTreeMap<String, HashMap<String, String>>, AppError> {
  read_csv_rows(&config.datasets_metadata_path).map(|rows| {
    rows
      .into_iter()
      .map(|row| (value(&row, "dataset_id"), row))
      .collect()
  })
}

fn variable_rows(config: &RetrievalConfig) -> Result<Vec<HashMap<String, String>>, AppError> {
  read_csv_rows(&config.variables_metadata_path)
}

/// Returns availability years for a dataset.
///
/// # Errors
///
/// Returns [`AppError`] if the dataset is missing or metadata cannot be read.
pub fn dataset_availability(
  config: &RetrievalConfig,
  dataset_id: &str,
) -> Result<DatasetAvailability, AppError> {
  let datasets = datasets_by_id(config)?;
  let row = datasets
    .get(dataset_id)
    .ok_or_else(|| integration_error(format!("Dataset {dataset_id} not found")))?;
  Ok(DatasetAvailability {
    dataset_id: dataset_id.to_string(),
    name: value(row, "name"),
    available_years: parse_availability_years(&value(row, "availability")),
  })
}

fn crosswalk_item(
  row: &HashMap<String, String>,
  query_name: &str,
  datasets: &BTreeMap<String, HashMap<String, String>>,
) -> VariableCrosswalkItem {
  let dataset_id = value(row, "dataset_id");
  let dataset_name = datasets
    .get(&dataset_id)
    .map(|dataset| value(dataset, "name"))
    .unwrap_or_default();
  let available_years = datasets
    .get(&dataset_id)
    .map(|dataset| parse_availability_years(&value(dataset, "availability")))
    .unwrap_or_default();
  VariableCrosswalkItem {
    variable_name: query_name.to_string(),
    record_id: value(row, "variable_id"),
    dataset_id,
    dataset_name,
    definition: value(row, "definition"),
    available_years,
    source_url: value(row, "source_url"),
  }
}

/// Crosswalks variable names to dataset-specific rows.
///
/// # Errors
///
/// Returns [`AppError`] if metadata cannot be read.
pub fn crosswalk_variables(
  config: &RetrievalConfig,
  variables: &[String],
) -> Result<VariableCrosswalkResponse, AppError> {
  let datasets = datasets_by_id(config)?;
  let rows = variable_rows(config)?;
  let mut response = BTreeMap::new();
  for variable in variables {
    let trimmed = variable.trim();
    let normalized = trimmed.to_ascii_lowercase();
    let matches = rows
      .iter()
      .filter(|row| value(row, "variable_name").eq_ignore_ascii_case(&normalized))
      .map(|row| crosswalk_item(row, trimmed, &datasets))
      .collect::<Vec<_>>();
    response.insert(trimmed.to_string(), matches);
  }
  Ok(VariableCrosswalkResponse {
    variables: response,
  })
}

/// Generates a cohort dictionary keyed by requested variable names.
///
/// # Errors
///
/// Returns [`AppError`] if metadata cannot be read.
pub fn cohort_dictionary(
  config: &RetrievalConfig,
  variables: &[String],
) -> Result<BTreeMap<String, Vec<VariableCrosswalkItem>>, AppError> {
  crosswalk_variables(config, variables).map(|response| response.variables)
}

fn escape_xml(value: &str) -> String {
  value
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
}

/// Formats agent context for downstream prompts.
///
/// # Errors
///
/// Returns [`AppError`] for unsupported formats.
pub fn format_context(context: &AgentContext, format: &str) -> Result<String, AppError> {
  match format {
    "prompt" => Ok(format!(
      "=== CMS DOCUMENTATION CONTEXT ===\n{}",
      format_agent_context_text(context)
    )),
    "markdown" => {
      let mut lines = vec![format!(
        "### CMS Documentation Context\n\nQuery: {}",
        context.query
      )];
      for (index, entry) in context.entries.iter().enumerate() {
        lines.push(format!(
          "#### {}. {} ({})\n\n{}\n\n**Source URL**: {}",
          index + 1,
          entry.title,
          entry.record_type,
          entry.snippet,
          entry.source_url
        ));
      }
      Ok(lines.join("\n\n"))
    }
    "xml" => {
      let mut output = format!(
        "<documentation_context><query>{}</query>",
        escape_xml(&context.query)
      );
      for entry in &context.entries {
        let _ = write!(
          output,
          "<record id=\"{}\" type=\"{}\" title=\"{}\"><source_url>{}</source_url><excerpt>{}</excerpt></record>",
          escape_xml(&entry.record_id),
          escape_xml(&entry.record_type),
          escape_xml(&entry.title),
          escape_xml(&entry.source_url),
          escape_xml(&entry.snippet)
        );
      }
      output.push_str("</documentation_context>");
      Ok(output)
    }
    _ => Err(integration_error(format!(
      "unsupported context format: {format}"
    ))),
  }
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), AppError> {
  if path.is_file() {
    files.push(path.to_path_buf());
  } else if path.is_dir() {
    for entry in fs::read_dir(path)
      .map_err(|error| integration_error(format!("failed to read {}: {error}", path.display())))?
    {
      collect_files(
        &entry
          .map_err(|error| integration_error(error.to_string()))?
          .path(),
        files,
      )?;
    }
  }
  Ok(())
}

/// Scans code files for dataset IDs, variable names, and additional keywords.
///
/// # Errors
///
/// Returns [`AppError`] if inputs cannot be read.
pub fn scan_codebase_caveats(
  config: &RetrievalConfig,
  paths: &[PathBuf],
  keywords: &[String],
) -> Result<CaveatScanResponse, AppError> {
  let mut files = Vec::new();
  for path in paths {
    collect_files(path, &mut files)?;
  }
  let mut haystack = String::new();
  for file in files {
    haystack.push_str(
      &fs::read_to_string(&file).map_err(|error| {
        integration_error(format!("failed to read {}: {error}", file.display()))
      })?,
    );
    haystack.push('\n');
  }
  let mut terms = keywords
    .iter()
    .map(|value| value.trim().to_string())
    .collect::<Vec<_>>();
  for row in variable_rows(config)? {
    terms.push(value(&row, "variable_name"));
  }
  for dataset_id in datasets_by_id(config)?.keys() {
    terms.push(dataset_id.clone());
  }
  let rows = variable_rows(config)?;
  terms.retain(|term| !term.is_empty());
  terms.sort();
  terms.dedup();
  let lower_haystack = haystack.to_ascii_lowercase();
  let mut matches = BTreeMap::new();
  for term in terms {
    if lower_haystack.contains(&term.to_ascii_lowercase()) {
      let items = if rows
        .iter()
        .any(|row| value(row, "variable_name").eq_ignore_ascii_case(&term))
      {
        crosswalk_variables(config, std::slice::from_ref(&term))?
          .variables
          .remove(&term)
          .unwrap_or_default()
      } else {
        Vec::new()
      };
      matches.insert(term, items);
    }
  }
  Ok(CaveatScanResponse { matches })
}

/// Builds and formats context from retrieval.
///
/// # Errors
///
/// Returns [`AppError`] if retrieval or formatting fails.
pub fn run_format_context(
  config: &RetrievalConfig,
  query: &str,
  limit: usize,
  format: &str,
) -> Result<String, AppError> {
  let results = retrieval::run_retrieval(config, query, limit)?;
  let context = crate::agent_context::build_agent_context(query, results);
  format_context(&context, format)
}

#[must_use]
pub fn record_type_name(record_type: RecordType) -> &'static str {
  record_type.as_str()
}
