//! Variable-level metadata extraction from parsed chunks and archived variable pages.

use crate::config::VariableExtractionConfig;
use crate::error::AppError;
use crate::records::{
  ArchiveManifestRow, CanonicalVariableRow, ChunkMetadata, DataSourceVariableEdgeRow,
  VariableEdgeRow, VariableMetadataRow,
};
use regex::Regex;
use scraper::{Html, Selector};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use url::Url;

const VARIABLE_HEADERS: &[&str] = &[
  "variable_id",
  "variable_name",
  "dataset_id",
  "definition",
  "aliases",
  "years",
  "source_document",
  "source_url",
  "page",
  "chunk_id",
  "extraction_notes",
];
const VARIABLE_EDGE_HEADERS: &[&str] = &[
  "source_id",
  "target_id",
  "relationship",
  "source_url",
  "source_document",
  "page",
  "chunk_id",
];
const CANONICAL_VARIABLE_HEADERS: &[&str] = &[
  "variable_id",
  "variable_name",
  "variable_label",
  "definition",
  "source",
  "source_url",
  "source_document",
  "extraction_notes",
];
const DATA_SOURCE_VARIABLE_EDGE_HEADERS: &[&str] = &[
  "source_id",
  "target_id",
  "relationship",
  "source_url",
  "source_document",
  "variable_url",
  "variable_document",
  "evidence_type",
  "page",
  "chunk_id",
];

/// A recoverable failure associated with one chunk or archived document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariableExtractionFailure {
  pub chunk_id: String,
  pub source_document: String,
  pub reason: String,
}

/// Complete variable extraction outcome, including partial failures and output location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariableExtractionResult {
  pub chunks_read: usize,
  pub variables: Vec<VariableMetadataRow>,
  pub edges: Vec<VariableEdgeRow>,
  pub canonical_variables: Vec<CanonicalVariableRow>,
  pub data_source_variable_edges: Vec<DataSourceVariableEdgeRow>,
  pub skipped_candidates: usize,
  pub failures: Vec<VariableExtractionFailure>,
  pub summary_path: PathBuf,
}

fn variable_pattern() -> &'static Regex {
  static PATTERN: OnceLock<Regex> = OnceLock::new();
  PATTERN.get_or_init(|| {
    Regex::new(r"\b[A-Z][A-Z0-9]{1,}(?:_[A-Z0-9]+)+\b")
      .expect("the variable regex is a static invariant")
  })
}

fn year_pattern() -> &'static Regex {
  static PATTERN: OnceLock<Regex> = OnceLock::new();
  PATTERN.get_or_init(|| {
    Regex::new(r"\b(?:19|20)\d{2}\b").expect("the year regex is a static invariant")
  })
}

fn alias_pattern() -> &'static Regex {
  static PATTERN: OnceLock<Regex> = OnceLock::new();
  PATTERN.get_or_init(|| {
    Regex::new(r"(?i)\b(?:also known as|aka|alias(?:es)?|formerly)\b[:\s]+([^.;\n]+)")
      .expect("the alias regex is a static invariant")
  })
}

fn alias_separator_pattern() -> &'static Regex {
  static PATTERN: OnceLock<Regex> = OnceLock::new();
  PATTERN.get_or_init(|| {
    Regex::new(r",|\bor\b").expect("the alias separator regex is a static invariant")
  })
}

fn slugify(value: &str) -> String {
  let mut slug = String::new();
  let mut separated = false;
  for character in value.chars().flat_map(char::to_lowercase) {
    if character.is_ascii_alphanumeric() {
      if separated && !slug.is_empty() {
        slug.push('-');
      }
      slug.push(character);
      separated = false;
    } else {
      separated = true;
    }
  }
  if slug.is_empty() {
    "unknown".to_string()
  } else {
    slug
  }
}

fn clean_definition(value: &str) -> String {
  value
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
    .trim_matches([' ', '.', ';', ':', '-', '–', '—'])
    .to_string()
}

fn table_cells(line: &str) -> Vec<&str> {
  if !line.contains('|') {
    return Vec::new();
  }
  line
    .trim()
    .trim_matches('|')
    .split('|')
    .map(str::trim)
    .filter(|cell| !cell.is_empty())
    .collect()
}

fn is_table_separator(cell: &str) -> bool {
  let trimmed = cell.trim_matches(':');
  trimmed.len() >= 2 && trimmed.chars().all(|character| character == '-')
}

fn candidate_definition(line: &str, variable_name: &str) -> Option<String> {
  let cells = table_cells(line);
  if cells.len() >= 2
    && !cells.iter().all(|cell| is_table_separator(cell))
    && let Some(index) = cells.iter().position(|cell| *cell == variable_name)
  {
    for candidate in &cells[index + 1..] {
      let lower = candidate.to_ascii_lowercase();
      if *candidate != variable_name
        && !is_table_separator(candidate)
        && !matches!(
          lower.as_str(),
          "variable name" | "sas name" | "short name" | "long name"
        )
      {
        let definition = clean_definition(candidate);
        if !definition.is_empty() {
          return Some(definition);
        }
      }
    }
  }

  let variable_match = variable_pattern()
    .find_iter(line)
    .find(|candidate| candidate.as_str() == variable_name)?;
  let after = line[variable_match.end()..].trim();
  if after.is_empty() {
    return None;
  }
  for separator in ["-", ":", "–", "—", "="] {
    if let Some(definition) = after.strip_prefix(separator) {
      if !definition.starts_with(char::is_whitespace) {
        continue;
      }
      let definition = clean_definition(definition);
      return (!definition.is_empty()).then_some(definition);
    }
  }
  let lower = after.to_ascii_lowercase();
  for prefix in ["means ", "indicates ", "identifies ", "is "] {
    if lower.starts_with(prefix) {
      let definition = clean_definition(&after[prefix.len()..]);
      return (!definition.is_empty()).then_some(definition);
    }
  }
  None
}

fn extract_aliases(line: &str) -> Option<String> {
  let mut aliases = BTreeSet::new();
  for captures in alias_pattern().captures_iter(line) {
    for alias in alias_separator_pattern().split(&captures[1]) {
      let alias = alias.trim_matches([' ', '.', ';', '(', ')']);
      if !alias.is_empty() && !year_pattern().is_match(alias) {
        aliases.insert(alias.to_string());
      }
    }
  }
  (!aliases.is_empty()).then(|| aliases.into_iter().collect::<Vec<_>>().join("|"))
}

fn extract_years(line: &str) -> Option<String> {
  let years: BTreeSet<&str> = year_pattern()
    .find_iter(line)
    .map(|matched| matched.as_str())
    .collect();
  (!years.is_empty()).then(|| years.into_iter().collect::<Vec<_>>().join("|"))
}

/// Extracts conservative variable records from one parsed chunk.
#[must_use]
pub fn extract_variables_from_chunk(chunk: &ChunkMetadata) -> (Vec<VariableMetadataRow>, usize) {
  let mut rows = Vec::new();
  let mut skipped = 0;
  let mut seen = BTreeSet::new();

  for raw_line in chunk.text.lines() {
    let line = raw_line.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.is_empty() {
      continue;
    }
    let names: BTreeSet<&str> = variable_pattern()
      .find_iter(&line)
      .map(|matched| matched.as_str())
      .collect();
    for variable_name in names {
      if !seen.insert(variable_name.to_string()) {
        continue;
      }
      let Some(definition) = candidate_definition(&line, variable_name) else {
        skipped += 1;
        continue;
      };
      rows.push(VariableMetadataRow {
        variable_id: format!(
          "{}__var__{}",
          slugify(&chunk.dataset),
          slugify(variable_name)
        ),
        variable_name: variable_name.to_string(),
        dataset_id: chunk.dataset.clone(),
        definition,
        aliases: extract_aliases(&line),
        years: extract_years(&line),
        source_document: chunk.source_document.clone(),
        source_url: chunk.url.clone(),
        page: chunk.page,
        chunk_id: chunk.chunk_id.clone(),
        extraction_notes: None,
      });
    }
  }
  (rows, skipped)
}

fn source_priority(row: &VariableMetadataRow) -> u8 {
  let source_url = row.source_url.to_ascii_lowercase();
  let source_document = row.source_document.to_ascii_lowercase();
  let is_html = has_extension(&source_document, "html")
    || row
      .extraction_notes
      .as_deref()
      .is_some_and(|notes| notes.to_ascii_lowercase().contains("text/html"));
  if is_html && source_url.contains("/data-documentation") {
    0
  } else if is_html {
    1
  } else if has_extension(&source_document, "xlsx") || has_extension(&source_url, "xlsx") {
    2
  } else {
    3
  }
}

fn has_extension(value: &str, expected: &str) -> bool {
  Path::new(value)
    .extension()
    .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

fn deduplicate_variables(rows: Vec<VariableMetadataRow>) -> Vec<VariableMetadataRow> {
  let mut unique = BTreeMap::new();
  for row in rows {
    let replace = unique
      .get(&row.variable_id)
      .is_none_or(|existing: &VariableMetadataRow| {
        source_priority(&row) < source_priority(existing)
          || (source_priority(&row) == source_priority(existing)
            && row.definition.len() > existing.definition.len())
      });
    if replace {
      unique.insert(row.variable_id.clone(), row);
    }
  }
  let mut values: Vec<_> = unique.into_values().collect();
  values.sort_by(|left, right| {
    (&left.dataset_id, &left.variable_name).cmp(&(&right.dataset_id, &right.variable_name))
  });
  values
}

fn read_chunks_jsonl(
  path: &Path,
) -> Result<(Vec<ChunkMetadata>, Vec<VariableExtractionFailure>), AppError> {
  let file = File::open(path).map_err(|error| {
    AppError::RecordParseError(format!(
      "failed to open chunks JSONL {}: {error}",
      path.display()
    ))
  })?;
  let mut chunks = Vec::new();
  let mut failures = Vec::new();
  for (index, line) in BufReader::new(file).lines().enumerate() {
    let line = line.map_err(|error| {
      AppError::RecordParseError(format!(
        "failed to read chunks JSONL {}: {error}",
        path.display()
      ))
    })?;
    if line.trim().is_empty() {
      continue;
    }
    match serde_json::from_str(&line) {
      Ok(chunk) => chunks.push(chunk),
      Err(error) => failures.push(VariableExtractionFailure {
        chunk_id: format!("line-{}", index + 1),
        source_document: String::new(),
        reason: format!("failed to parse chunk JSON: {error}"),
      }),
    }
  }
  Ok((chunks, failures))
}

fn selector(value: &str) -> Result<Selector, AppError> {
  Selector::parse(value).map_err(|error| {
    AppError::RecordParseError(format!("failed to build HTML selector '{value}': {error}"))
  })
}

fn element_text(element: scraper::ElementRef<'_>) -> String {
  element
    .text()
    .flat_map(str::split_whitespace)
    .collect::<Vec<_>>()
    .join(" ")
}

fn field_from_rows(document: &Html, names: &[&str]) -> Result<String, AppError> {
  let row_selector = selector("tr")?;
  let cell_selector = selector("th, td")?;
  for row in document.select(&row_selector) {
    let cells: Vec<String> = row.select(&cell_selector).map(element_text).collect();
    if cells.len() >= 2 {
      let label = cells[0].trim_matches([' ', ':']).to_ascii_lowercase();
      if names.contains(&label.as_str()) {
        return Ok(cells[1].trim().to_string());
      }
    }
  }
  Ok(String::new())
}

fn dataset_id_from_file_url(value: &str) -> Option<String> {
  let url = Url::parse(value).ok()?;
  let parts: Vec<&str> = url
    .path_segments()?
    .filter(|part| !part.is_empty())
    .collect();
  (parts.len() >= 3 && parts[0] == "cms-data" && parts[1] == "files").then(|| slugify(parts[2]))
}

fn canonical_id_from_url(value: &str) -> String {
  Url::parse(value)
    .ok()
    .and_then(|url| {
      url
        .path_segments()
        .and_then(|mut parts| parts.next_back())
        .map(slugify)
    })
    .unwrap_or_else(|| "unknown".to_string())
}

fn extract_canonical_variable(
  row: &ArchiveManifestRow,
) -> Result<Option<(CanonicalVariableRow, Vec<DataSourceVariableEdgeRow>)>, AppError> {
  if row.archive_state != "archived" || row.resource_kind != "variable_page" {
    return Ok(None);
  }
  let Some(local_path) = row.local_path.as_deref() else {
    return Ok(None);
  };
  if !Path::new(local_path).is_file() {
    return Ok(None);
  }
  let html_bytes = fs::read(local_path).map_err(|error| {
    AppError::RecordParseError(format!(
      "failed to read variable page {local_path}: {error}"
    ))
  })?;
  let html = String::from_utf8_lossy(&html_bytes);
  let document = Html::parse_document(&html);
  let h1_selector = selector("h1")?;
  let title_selector = selector("title")?;
  let raw_label = document
    .select(&h1_selector)
    .next()
    .or_else(|| document.select(&title_selector).next())
    .map(element_text)
    .unwrap_or_default();
  let variable_label = raw_label
    .strip_suffix(" | ResDAC")
    .unwrap_or(&raw_label)
    .trim()
    .to_string();
  let mut variable_name = field_from_rows(&document, &["sas name", "variable name", "name"])?;
  if variable_name.is_empty() {
    variable_name = variable_pattern()
      .find(&html)
      .map_or_else(String::new, |matched| matched.as_str().to_string());
  }
  let mut definition = field_from_rows(&document, &["definition", "description"])?;
  if definition.is_empty() && !variable_name.is_empty() {
    let page_text = element_text(document.root_element());
    for prefix in ["Definition", "Description"] {
      if let Some((_, after)) = page_text.split_once(prefix) {
        definition = clean_definition(after.trim_start_matches([' ', ':', '-']));
        if !definition.is_empty() {
          break;
        }
      }
    }
  }
  let variable = CanonicalVariableRow {
    variable_id: canonical_id_from_url(&row.url),
    variable_name: variable_name.clone(),
    variable_label,
    definition,
    source: "resdac_variable_page".to_string(),
    source_url: row.url.clone(),
    source_document: local_path.to_string(),
    extraction_notes: variable_name
      .is_empty()
      .then(|| "variable name not found on page".to_string()),
  };

  let base_url = Url::parse(&row.url).map_err(|error| {
    AppError::RecordParseError(format!("invalid variable page URL {}: {error}", row.url))
  })?;
  let link_selector = selector("a[href]")?;
  let mut edges = BTreeMap::new();
  for link in document.select(&link_selector) {
    let Some(href) = link.value().attr("href") else {
      continue;
    };
    let Ok(url) = base_url.join(href) else {
      continue;
    };
    let url_string = url.to_string();
    let Some(source_id) = dataset_id_from_file_url(&url_string) else {
      continue;
    };
    edges.insert(
      url_string.clone(),
      DataSourceVariableEdgeRow {
        source_id,
        target_id: variable.variable_id.clone(),
        relationship: "contains".to_string(),
        source_url: url_string,
        source_document: String::new(),
        variable_url: row.url.clone(),
        variable_document: local_path.to_string(),
        evidence_type: "variable_page_containing_file".to_string(),
        page: None,
        chunk_id: String::new(),
      },
    );
  }
  Ok(Some((variable, edges.into_values().collect())))
}

fn read_canonical_variables(
  manifest_path: &Path,
) -> (
  Vec<CanonicalVariableRow>,
  Vec<DataSourceVariableEdgeRow>,
  Vec<VariableExtractionFailure>,
) {
  if !manifest_path.is_file() {
    return (Vec::new(), Vec::new(), Vec::new());
  }
  let mut reader = match csv::Reader::from_path(manifest_path) {
    Ok(reader) => reader,
    Err(error) => {
      return (
        Vec::new(),
        Vec::new(),
        vec![VariableExtractionFailure {
          chunk_id: String::new(),
          source_document: String::new(),
          reason: format!("failed to read archive manifest: {error}"),
        }],
      );
    }
  };
  let mut variables = BTreeMap::new();
  let mut edges = BTreeMap::new();
  let mut failures = Vec::new();
  for result in reader.deserialize::<ArchiveManifestRow>() {
    let row = match result {
      Ok(row) => row,
      Err(error) => {
        failures.push(VariableExtractionFailure {
          chunk_id: String::new(),
          source_document: String::new(),
          reason: format!("failed to read archive manifest: {error}"),
        });
        continue;
      }
    };
    match extract_canonical_variable(&row) {
      Ok(Some((variable, variable_edges))) => {
        variables.insert(variable.variable_id.clone(), variable);
        for edge in variable_edges {
          edges.insert(
            (
              edge.source_id.clone(),
              edge.target_id.clone(),
              edge.source_url.clone(),
            ),
            edge,
          );
        }
      }
      Ok(None) => {}
      Err(error) => failures.push(VariableExtractionFailure {
        chunk_id: String::new(),
        source_document: row.local_path.unwrap_or_default(),
        reason: format!("failed to extract canonical variable page: {error}"),
      }),
    }
  }
  (
    variables.into_values().collect(),
    edges.into_values().collect(),
    failures,
  )
}

fn resolve_citations(
  variables: Vec<VariableMetadataRow>,
  canonical_variables: &[CanonicalVariableRow],
  edges: &[DataSourceVariableEdgeRow],
) -> Vec<VariableMetadataRow> {
  let names: BTreeMap<&str, &str> = canonical_variables
    .iter()
    .filter(|variable| !variable.variable_name.is_empty())
    .map(|variable| {
      (
        variable.variable_id.as_str(),
        variable.variable_name.as_str(),
      )
    })
    .collect();
  let resolved: BTreeMap<(String, String), (&str, &str)> = edges
    .iter()
    .filter_map(|edge| {
      names.get(edge.target_id.as_str()).map(|name| {
        (
          (
            edge.source_id.to_ascii_lowercase(),
            name.to_ascii_lowercase(),
          ),
          (edge.variable_url.as_str(), edge.variable_document.as_str()),
        )
      })
    })
    .collect();
  variables
    .into_iter()
    .map(|mut variable| {
      if let Some((source_url, source_document)) = resolved.get(&(
        variable.dataset_id.to_ascii_lowercase(),
        variable.variable_name.to_ascii_lowercase(),
      )) {
        if !source_url.is_empty() {
          variable.source_url = (*source_url).to_string();
        }
        if !source_document.is_empty() {
          variable.source_document = (*source_document).to_string();
        }
      }
      variable
    })
    .collect()
}

fn edge_for_variable(row: &VariableMetadataRow) -> VariableEdgeRow {
  VariableEdgeRow {
    source_id: row.dataset_id.clone(),
    target_id: row.variable_id.clone(),
    relationship: "contains".to_string(),
    source_url: row.source_url.clone(),
    source_document: row.source_document.clone(),
    page: row.page,
    chunk_id: row.chunk_id.clone(),
  }
}

fn write_csv<T: Serialize>(path: &Path, headers: &[&str], rows: &[T]) -> Result<(), AppError> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).map_err(|error| {
      AppError::RecordParseError(format!(
        "failed to create output directory {}: {error}",
        parent.display()
      ))
    })?;
  }
  let file = File::create(path).map_err(|error| {
    AppError::RecordParseError(format!("failed to create CSV {}: {error}", path.display()))
  })?;
  let mut writer = csv::WriterBuilder::new()
    .has_headers(false)
    .from_writer(file);
  writer.write_record(headers).map_err(|error| {
    AppError::RecordParseError(format!(
      "failed to write CSV header {}: {error}",
      path.display()
    ))
  })?;
  for row in rows {
    writer.serialize(row).map_err(|error| {
      AppError::RecordParseError(format!(
        "failed to write CSV row {}: {error}",
        path.display()
      ))
    })?;
  }
  writer.flush().map_err(|error| {
    AppError::RecordParseError(format!("failed to flush CSV {}: {error}", path.display()))
  })
}

struct SummaryData<'a> {
  chunks_read: usize,
  variables: &'a [VariableMetadataRow],
  edges: &'a [VariableEdgeRow],
  canonical_variables: &'a [CanonicalVariableRow],
  data_source_edges: &'a [DataSourceVariableEdgeRow],
  skipped_candidates: usize,
  failures: &'a [VariableExtractionFailure],
}

fn write_summary(
  config: &VariableExtractionConfig,
  data: &SummaryData<'_>,
) -> Result<PathBuf, AppError> {
  fs::create_dir_all(&config.workspace_dir).map_err(|error| {
    AppError::RecordParseError(format!(
      "failed to create workspace {}: {error}",
      config.workspace_dir.display()
    ))
  })?;
  let summary_path = config.workspace_dir.join("07_variable_pack.md");
  let mut file = File::create(&summary_path).map_err(|error| {
    AppError::RecordParseError(format!(
      "failed to create variable summary {}: {error}",
      summary_path.display()
    ))
  })?;
  writeln!(file, "# Variable Pack\n").map_err(summary_error)?;
  writeln!(
    file,
    "- Parsed chunks input: {}",
    config.chunks_jsonl_path.display()
  )
  .map_err(summary_error)?;
  writeln!(file, "- Chunks read: {}", data.chunks_read).map_err(summary_error)?;
  writeln!(file, "- Variables: {}", data.variables.len()).map_err(summary_error)?;
  writeln!(file, "- Variable edges: {}", data.edges.len()).map_err(summary_error)?;
  writeln!(
    file,
    "- Canonical variables: {}",
    data.canonical_variables.len()
  )
  .map_err(summary_error)?;
  writeln!(
    file,
    "- Data source variable edges: {}",
    data.data_source_edges.len()
  )
  .map_err(summary_error)?;
  writeln!(file, "- Skipped candidates: {}", data.skipped_candidates).map_err(summary_error)?;
  writeln!(file, "- Failures: {}\n", data.failures.len()).map_err(summary_error)?;
  writeln!(file, "## Outputs\n").map_err(summary_error)?;
  writeln!(
    file,
    "- Variable metadata: {}",
    config.metadata_dir.join("variables.csv").display()
  )
  .map_err(summary_error)?;
  writeln!(
    file,
    "- Variable graph edges: {}",
    config.graph_dir.join("variable_edges.csv").display()
  )
  .map_err(summary_error)?;
  writeln!(
    file,
    "- Canonical variable metadata: {}",
    config
      .metadata_dir
      .join("canonical_variables.csv")
      .display()
  )
  .map_err(summary_error)?;
  writeln!(
    file,
    "- Data source variable graph edges: {}\n",
    config
      .graph_dir
      .join("data_source_variable_edges.csv")
      .display()
  )
  .map_err(summary_error)?;
  writeln!(file, "## Failures\n").map_err(summary_error)?;
  if data.failures.is_empty() {
    writeln!(file, "- None").map_err(summary_error)?;
  } else {
    writeln!(file, "| chunk_id | source_document | reason |").map_err(summary_error)?;
    writeln!(file, "| --- | --- | --- |").map_err(summary_error)?;
    for failure in data.failures.iter().take(25) {
      writeln!(
        file,
        "| {} | {} | {} |",
        failure.chunk_id,
        failure.source_document,
        failure.reason.replace('|', "\\|").replace('\n', " ")
      )
      .map_err(summary_error)?;
    }
    if data.failures.len() > 25 {
      writeln!(
        file,
        "\n- Additional failures omitted: {}",
        data.failures.len() - 25
      )
      .map_err(summary_error)?;
    }
  }
  Ok(summary_path)
}

#[allow(clippy::needless_pass_by_value)]
fn summary_error(error: std::io::Error) -> AppError {
  AppError::RecordParseError(format!("failed to write variable summary: {error}"))
}

/// Runs variable extraction and writes all metadata, graph, and summary artifacts.
///
/// Row-level failures are returned in the result after valid outputs are written.
///
/// # Errors
///
/// Returns [`AppError`] when a required input cannot be read or an output cannot be written.
pub fn run_variable_extraction(
  config: &VariableExtractionConfig,
) -> Result<VariableExtractionResult, AppError> {
  let (chunks, mut failures) = read_chunks_jsonl(&config.chunks_jsonl_path)?;
  let (canonical_variables, data_source_variable_edges, canonical_failures) =
    read_canonical_variables(&config.archive_manifest_path);
  failures.extend(canonical_failures);
  let mut extracted = Vec::new();
  let mut skipped_candidates = 0;
  for chunk in &chunks {
    if chunk.source_document.trim().is_empty() {
      failures.push(VariableExtractionFailure {
        chunk_id: chunk.chunk_id.clone(),
        source_document: String::new(),
        reason: "chunk has empty source_document".to_string(),
      });
      continue;
    }
    if !Path::new(&chunk.source_document).is_file() {
      failures.push(VariableExtractionFailure {
        chunk_id: chunk.chunk_id.clone(),
        source_document: chunk.source_document.clone(),
        reason: "source_document does not exist locally".to_string(),
      });
      continue;
    }
    let (rows, skipped) = extract_variables_from_chunk(chunk);
    extracted.extend(rows);
    skipped_candidates += skipped;
  }
  let variables = resolve_citations(
    deduplicate_variables(extracted),
    &canonical_variables,
    &data_source_variable_edges,
  );
  let edges: Vec<_> = variables.iter().map(edge_for_variable).collect();

  write_csv(
    &config.metadata_dir.join("variables.csv"),
    VARIABLE_HEADERS,
    &variables,
  )?;
  write_csv(
    &config.graph_dir.join("variable_edges.csv"),
    VARIABLE_EDGE_HEADERS,
    &edges,
  )?;
  write_csv(
    &config.metadata_dir.join("canonical_variables.csv"),
    CANONICAL_VARIABLE_HEADERS,
    &canonical_variables,
  )?;
  write_csv(
    &config.graph_dir.join("data_source_variable_edges.csv"),
    DATA_SOURCE_VARIABLE_EDGE_HEADERS,
    &data_source_variable_edges,
  )?;
  let summary_path = write_summary(
    config,
    &SummaryData {
      chunks_read: chunks.len(),
      variables: &variables,
      edges: &edges,
      canonical_variables: &canonical_variables,
      data_source_edges: &data_source_variable_edges,
      skipped_candidates,
      failures: &failures,
    },
  )?;

  Ok(VariableExtractionResult {
    chunks_read: chunks.len(),
    variables,
    edges,
    canonical_variables,
    data_source_variable_edges,
    skipped_candidates,
    failures,
    summary_path,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  fn variable(source_document: &str, source_url: &str, definition: &str) -> VariableMetadataRow {
    VariableMetadataRow {
      variable_id: "mbsf__var__bene-id".to_string(),
      variable_name: "BENE_ID".to_string(),
      dataset_id: "mbsf".to_string(),
      definition: definition.to_string(),
      aliases: None,
      years: None,
      source_document: source_document.to_string(),
      source_url: source_url.to_string(),
      page: None,
      chunk_id: "chunk-1".to_string(),
      extraction_notes: None,
    }
  }

  #[test]
  fn extracts_pipe_table_definition() {
    let chunk = ChunkMetadata {
      chunk_id: "chunk-1".to_string(),
      source_document: "source.html".to_string(),
      page: None,
      text: "| 1 | BENE_ID | CCW Encrypted Beneficiary ID Number |".to_string(),
      dataset: "medpar".to_string(),
      url: "https://resdac.org/cms-data/files/medpar/data-documentation".to_string(),
    };

    let (rows, skipped) = extract_variables_from_chunk(&chunk);

    assert_eq!(skipped, 0);
    assert_eq!(rows[0].definition, "CCW Encrypted Beneficiary ID Number");
  }

  #[test]
  fn deduplication_prefers_html_data_documentation() {
    let pdf = variable(
      "codebook.pdf",
      "https://example.test/codebook.pdf",
      "A longer PDF definition that loses to HTML data documentation",
    );
    let html = variable(
      "documentation.html",
      "https://resdac.org/cms-data/files/mbsf/data-documentation",
      "Beneficiary identifier",
    );

    assert_eq!(deduplicate_variables(vec![pdf, html.clone()]), vec![html]);
  }

  #[test]
  fn deduplication_prefers_longer_definition_at_equal_priority() {
    let short = variable(
      "source.txt",
      "https://example.test/source",
      "Beneficiary ID",
    );
    let long = variable(
      "source.txt",
      "https://example.test/source",
      "Encrypted CCW Beneficiary ID",
    );

    assert_eq!(deduplicate_variables(vec![short, long.clone()]), vec![long]);
  }
}
