//! Provenance and cross-artifact validation.

use crate::config::QAConfig;
use crate::error::AppError;
use crate::records::{
  ArchiveManifestRow, CanonicalVariableRow, DataSourceVariableEdgeRow, DatasetMetadataRow,
  DocumentEdgeRow, DocumentMetadataRow, OntologyEdgeRow, OntologyNodeRow, VariableEdgeRow,
  VariableMetadataRow,
};
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use url::Url;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QaSeverity {
  Warning,
  Error,
}

impl Display for QaSeverity {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      Self::Warning => "warning",
      Self::Error => "error",
    })
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QaVerdict {
  Pass,
  Fix,
  Redo,
}

impl Display for QaVerdict {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      Self::Pass => "PASS",
      Self::Fix => "FIX",
      Self::Redo => "REDO",
    })
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QaFinding {
  pub file: String,
  pub item_id: String,
  pub field: String,
  pub severity: QaSeverity,
  pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QaResult {
  pub verdict: QaVerdict,
  pub findings: Vec<QaFinding>,
  pub datasets_checked: usize,
  pub documents_checked: usize,
  pub variables_checked: usize,
  pub edges_checked: usize,
  pub summary_path: PathBuf,
}

impl QaResult {
  #[must_use]
  pub fn error_count(&self) -> usize {
    self
      .findings
      .iter()
      .filter(|f| f.severity == QaSeverity::Error)
      .count()
  }
  #[must_use]
  pub fn warning_count(&self) -> usize {
    self
      .findings
      .iter()
      .filter(|f| f.severity == QaSeverity::Warning)
      .count()
  }
}

fn finding(
  file: &str,
  item: &str,
  field: &str,
  severity: QaSeverity,
  message: impl Into<String>,
) -> QaFinding {
  QaFinding {
    file: file.to_string(),
    item_id: item.to_string(),
    field: field.to_string(),
    severity,
    message: message.into(),
  }
}

fn read_csv<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, AppError> {
  let mut reader =
    csv::Reader::from_path(path).map_err(|e| AppError::RecordParseError(e.to_string()))?;
  reader
    .deserialize()
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::RecordParseError(e.to_string()))
}

fn checksum(path: &Path) -> Result<String, std::io::Error> {
  let mut file = fs::File::open(path)?;
  let mut hash = Sha256::new();
  let mut buffer = [0_u8; 8192];
  loop {
    let count = file.read(&mut buffer)?;
    if count == 0 {
      break;
    }
    hash.update(&buffer[..count]);
  }
  Ok(format!("{:x}", hash.finalize()))
}

fn valid_url(value: &str) -> bool {
  Url::parse(value).is_ok_and(|url| url.has_host())
}

fn check_text(
  findings: &mut Vec<QaFinding>,
  file: &str,
  item: &str,
  field: &str,
  value: &str,
  required: bool,
) {
  if required && value.trim().is_empty() {
    findings.push(finding(
      file,
      item,
      field,
      QaSeverity::Error,
      format!("{field} is empty"),
    ));
  } else if value != value.trim() {
    findings.push(finding(
      file,
      item,
      field,
      QaSeverity::Warning,
      format!("{field} has leading/trailing whitespace: '{value}'"),
    ));
  }
}

fn check_url(findings: &mut Vec<QaFinding>, file: &str, item: &str, value: &str) {
  check_text(findings, file, item, "source_url", value, true);
  if !value.trim().is_empty() && !valid_url(value) {
    findings.push(finding(
      file,
      item,
      "source_url",
      QaSeverity::Error,
      format!("Invalid source_url: {value}"),
    ));
  }
}

fn check_evidence(
  findings: &mut Vec<QaFinding>,
  file: &str,
  item: &str,
  path_value: &str,
  expected: &str,
) {
  if path_value.trim().is_empty() {
    findings.push(finding(
      file,
      item,
      "local_path",
      QaSeverity::Error,
      "local_path is empty",
    ));
    return;
  }
  let path = Path::new(path_value);
  if !path.is_file() {
    findings.push(finding(
      file,
      item,
      "local_path",
      QaSeverity::Error,
      format!("Local file does not exist: {path_value}"),
    ));
  } else if !expected.trim().is_empty() {
    match checksum(path) {
      Ok(actual) if actual != expected => findings.push(finding(
        file,
        item,
        "sha256",
        QaSeverity::Error,
        format!("Checksum mismatch: expected {expected}, got {actual}"),
      )),
      Err(error) => findings.push(finding(
        file,
        item,
        "sha256",
        QaSeverity::Error,
        format!("Error computing checksum: {error}"),
      )),
      _ => {}
    }
  }
}

fn check_source_document(
  findings: &mut Vec<QaFinding>,
  file: &str,
  item: &str,
  field: &str,
  value: &str,
) {
  let path = value.trim();
  if path.is_empty() {
    findings.push(finding(
      file,
      item,
      field,
      QaSeverity::Error,
      format!("Empty {field}"),
    ));
  } else if !Path::new(path).is_file() {
    findings.push(finding(
      file,
      item,
      field,
      QaSeverity::Error,
      format!("Source document does not exist: {path}"),
    ));
  }
}

fn check_optional_evidence(
  findings: &mut Vec<QaFinding>,
  file: &str,
  item: &str,
  path_value: &str,
  expected: &str,
) {
  let value = path_value.trim();
  if value.is_empty() {
    return;
  }
  let path = Path::new(value);
  if !path.is_file() {
    findings.push(finding(
      file,
      item,
      "local_path",
      QaSeverity::Warning,
      format!("Local file does not exist: {value}"),
    ));
  } else if !expected.trim().is_empty() && checksum(path).is_ok_and(|actual| actual != expected) {
    findings.push(finding(
      file,
      item,
      "sha256",
      QaSeverity::Warning,
      "Checksum mismatch",
    ));
  }
}

fn check_manifest_checksum(
  findings: &mut Vec<QaFinding>,
  file: &str,
  item: &str,
  path: &str,
  manifest_row: &ArchiveManifestRow,
) {
  let Ok(actual) = checksum(Path::new(path.trim())) else {
    return;
  };
  if manifest_row.sha256.as_deref() != Some(actual.as_str()) {
    findings.push(finding(
      file,
      item,
      "sha256",
      QaSeverity::Error,
      format!(
        "Checksum mismatch with manifest. Manifest: {:?}, Actual: {actual}",
        manifest_row.sha256
      ),
    ));
  }
}

fn duplicate(
  findings: &mut Vec<QaFinding>,
  seen: &mut HashSet<String>,
  file: &str,
  field: &str,
  id: &str,
) {
  if !seen.insert(id.to_string()) {
    findings.push(finding(
      file,
      id,
      field,
      QaSeverity::Error,
      format!("Duplicate {field} encountered: {id}"),
    ));
  }
}

fn verdict(findings: &[QaFinding]) -> QaVerdict {
  let errors: Vec<_> = findings
    .iter()
    .filter(|f| f.severity == QaSeverity::Error)
    .collect();
  if errors.is_empty() {
    return QaVerdict::Pass;
  }
  let major = [
    "csv_parsing",
    "file_existence",
    "dataset_id",
    "document_id",
    "dataset_count",
    "document_count",
    "node_class",
    "node_id",
    "variable_id",
    "variable_name",
    "source_id",
    "target_id",
    "source_document",
    "chunk_id",
  ];
  if errors.len() > 5
    || errors.iter().any(|f| {
      major.contains(&f.field.as_str()) || f.message.contains("does not exist in datasets metadata")
    })
  {
    QaVerdict::Redo
  } else {
    QaVerdict::Fix
  }
}

fn write_summary(config: &QAConfig, result: &QaResult) -> Result<PathBuf, AppError> {
  fs::create_dir_all(&config.workspace_dir)
    .map_err(|e| AppError::PathResolutionError(e.to_string()))?;
  let path = config.workspace_dir.join("06_qa_review.md");
  let mut lines = vec![
    "# QA Review".to_string(),
    String::new(),
    format!("- Verdict: **{}**", result.verdict),
    String::new(),
    "## Metadata Checked".to_string(),
    String::new(),
    format!("- Datasets Checked: {}", result.datasets_checked),
    format!("- Documents Checked: {}", result.documents_checked),
    format!("- Variables Checked: {}", result.variables_checked),
    format!("- Edges Checked: {}", result.edges_checked),
    format!("- Total Findings: {}", result.findings.len()),
    format!("  - Errors: {}", result.error_count()),
    format!("  - Warnings: {}", result.warning_count()),
    String::new(),
    "## Findings".to_string(),
    String::new(),
  ];
  if result.findings.is_empty() {
    lines.push("- No issues identified.".to_string());
  } else {
    lines.push("| File | Item ID | Field | Severity | Message |".to_string());
    lines.push("| --- | --- | --- | --- | --- |".to_string());
    for item in &result.findings {
      lines.push(format!(
        "| {} | {} | {} | {} | {} |",
        item.file,
        item.item_id,
        item.field,
        item.severity,
        item.message.replace('|', "\\|").replace('\n', " ")
      ));
    }
  }
  fs::write(&path, format!("{}\n", lines.join("\n")))
    .map_err(|e| AppError::PathResolutionError(e.to_string()))?;
  Ok(path)
}

fn finish(
  config: &QAConfig,
  findings: Vec<QaFinding>,
  datasets: usize,
  documents: usize,
  variables: usize,
  edges: usize,
) -> Result<QaResult, AppError> {
  let mut result = QaResult {
    verdict: verdict(&findings),
    findings,
    datasets_checked: datasets,
    documents_checked: documents,
    variables_checked: variables,
    edges_checked: edges,
    summary_path: PathBuf::new(),
  };
  result.summary_path = write_summary(config, &result)?;
  Ok(result)
}

fn optional_csv<T: DeserializeOwned>(
  path: &Path,
  label: &str,
  findings: &mut Vec<QaFinding>,
) -> Vec<T> {
  if !path.is_file() {
    return Vec::new();
  }
  match read_csv(path) {
    Ok(rows) => rows,
    Err(error) => {
      findings.push(finding(
        &path.display().to_string(),
        "header/parse",
        "csv_parsing",
        QaSeverity::Error,
        format!("Failed to read {label}: {error}"),
      ));
      Vec::new()
    }
  }
}

/// Validates all provenance artifacts and writes a deterministic QA review.
///
/// # Errors
///
/// Returns an error when an input adapter or report write fails unexpectedly.
#[allow(clippy::too_many_lines)]
pub fn run_qa(config: &QAConfig) -> Result<QaResult, AppError> {
  let mut findings = Vec::new();
  for (label, path) in [
    ("datasets metadata", &config.datasets_metadata_path),
    ("documents metadata", &config.documents_metadata_path),
    ("archive manifest", &config.archive_manifest_path),
  ] {
    if !path.is_file() {
      findings.push(finding(
        &path.display().to_string(),
        "N/A",
        "file_existence",
        QaSeverity::Error,
        format!("Required {label} file is missing"),
      ));
    }
  }
  if !findings.is_empty() {
    return finish(config, findings, 0, 0, 0, 0);
  }

  let manifest: Vec<ArchiveManifestRow> = match read_csv(&config.archive_manifest_path) {
    Ok(rows) => rows,
    Err(error) => {
      findings.push(finding(
        &config.archive_manifest_path.display().to_string(),
        "header/parse",
        "csv_parsing",
        QaSeverity::Error,
        format!("Failed to read archive manifest: {error}"),
      ));
      return finish(config, findings, 0, 0, 0, 0);
    }
  };
  let datasets: Vec<DatasetMetadataRow> = match read_csv(&config.datasets_metadata_path) {
    Ok(rows) => rows,
    Err(error) => {
      findings.push(finding(
        &config.datasets_metadata_path.display().to_string(),
        "header/parse",
        "csv_parsing",
        QaSeverity::Error,
        format!("Failed to read datasets metadata: {error}"),
      ));
      return finish(config, findings, 0, 0, 0, 0);
    }
  };
  let documents: Vec<DocumentMetadataRow> = match read_csv(&config.documents_metadata_path) {
    Ok(rows) => rows,
    Err(error) => {
      findings.push(finding(
        &config.documents_metadata_path.display().to_string(),
        "header/parse",
        "csv_parsing",
        QaSeverity::Error,
        format!("Failed to read documents metadata: {error}"),
      ));
      return finish(config, findings, 0, 0, 0, 0);
    }
  };
  let variables: Vec<VariableMetadataRow> = optional_csv(
    &config.variables_metadata_path,
    "variables metadata",
    &mut findings,
  );
  let canonical: Vec<CanonicalVariableRow> = optional_csv(
    &config.canonical_variables_metadata_path,
    "canonical variables metadata",
    &mut findings,
  );
  let variable_edges: Vec<VariableEdgeRow> =
    optional_csv(&config.variable_edges_path, "variable edges", &mut findings);
  let data_edges: Vec<DataSourceVariableEdgeRow> = optional_csv(
    &config.data_source_variable_edges_path,
    "data source variable edges",
    &mut findings,
  );
  let nodes: Vec<OntologyNodeRow> =
    optional_csv(&config.ontology_nodes_path, "ontology nodes", &mut findings);
  let ontology_edges: Vec<OntologyEdgeRow> =
    optional_csv(&config.ontology_edges_path, "ontology edges", &mut findings);
  let document_edges: Vec<DocumentEdgeRow> = if config.document_edges_path.is_file() {
    optional_csv(&config.document_edges_path, "document edges", &mut findings)
  } else {
    findings.push(finding(
      &config.document_edges_path.display().to_string(),
      "N/A",
      "file_existence",
      QaSeverity::Warning,
      "Document edges CSV file is missing",
    ));
    Vec::new()
  };

  if datasets.is_empty() {
    findings.push(finding(
      "datasets.csv",
      "N/A",
      "dataset_count",
      QaSeverity::Error,
      "Datasets metadata file has zero rows of data",
    ));
  }
  if documents.is_empty() {
    findings.push(finding(
      "documents.csv",
      "N/A",
      "document_count",
      QaSeverity::Error,
      "Documents metadata file has zero rows of data",
    ));
  }
  let manifest_by_url: HashMap<_, _> = manifest.iter().map(|row| (row.url.as_str(), row)).collect();
  let dataset_ids: HashSet<_> = datasets.iter().map(|row| row.dataset_id.as_str()).collect();
  let document_ids: HashSet<_> = documents
    .iter()
    .map(|row| row.document_id.as_str())
    .collect();
  let variable_ids: HashSet<_> = variables
    .iter()
    .map(|row| row.variable_id.as_str())
    .collect();
  let canonical_ids: HashSet<_> = canonical
    .iter()
    .map(|row| row.variable_id.as_str())
    .collect();
  let node_ids: HashSet<_> = nodes.iter().map(|row| row.node_id.as_str()).collect();

  let mut seen = HashSet::new();
  for row in &datasets {
    check_text(
      &mut findings,
      "datasets.csv",
      &row.dataset_id,
      "dataset_id",
      &row.dataset_id,
      true,
    );
    duplicate(
      &mut findings,
      &mut seen,
      "datasets.csv",
      "dataset_id",
      &row.dataset_id,
    );
    check_url(
      &mut findings,
      "datasets.csv",
      &row.dataset_id,
      &row.source_url,
    );
    check_evidence(
      &mut findings,
      "datasets.csv",
      &row.dataset_id,
      &row.local_path,
      &row.sha256,
    );
    match manifest_by_url.get(row.source_url.as_str()) {
      None => findings.push(finding(
        "datasets.csv",
        &row.dataset_id,
        "source_url",
        QaSeverity::Error,
        format!(
          "source_url not found in archive manifest: {}",
          row.source_url
        ),
      )),
      Some(item) => {
        if item.archive_state != "archived" {
          findings.push(finding(
            "datasets.csv",
            &row.dataset_id,
            "source_url",
            QaSeverity::Error,
            format!(
              "source_url archive state is '{}' (expected 'archived') for dataset: {}",
              item.archive_state, row.dataset_id
            ),
          ));
        }
        check_manifest_checksum(
          &mut findings,
          "datasets.csv",
          &row.dataset_id,
          &row.local_path,
          item,
        );
      }
    }
  }
  seen.clear();
  for row in &documents {
    check_text(
      &mut findings,
      "documents.csv",
      &row.document_id,
      "document_id",
      &row.document_id,
      true,
    );
    duplicate(
      &mut findings,
      &mut seen,
      "documents.csv",
      "document_id",
      &row.document_id,
    );
    if !dataset_ids.contains(row.dataset_id.as_str()) {
      findings.push(finding(
        "documents.csv",
        &row.document_id,
        "dataset_id",
        QaSeverity::Error,
        format!(
          "dataset_id does not exist in datasets metadata: {}",
          row.dataset_id
        ),
      ));
    }
    check_url(
      &mut findings,
      "documents.csv",
      &row.document_id,
      &row.source_url,
    );
    check_evidence(
      &mut findings,
      "documents.csv",
      &row.document_id,
      &row.local_path,
      &row.sha256,
    );
    match manifest_by_url.get(row.source_url.as_str()) {
      None => findings.push(finding(
        "documents.csv",
        &row.document_id,
        "source_url",
        QaSeverity::Error,
        format!(
          "source_url not found in archive manifest: {}",
          row.source_url
        ),
      )),
      Some(item) => {
        if item.archive_state != "archived" {
          findings.push(finding(
            "documents.csv",
            &row.document_id,
            "source_url",
            QaSeverity::Error,
            format!(
              "source_url archive state is '{}' (expected 'archived') for document: {}",
              item.archive_state, row.document_id
            ),
          ));
        }
        check_manifest_checksum(
          &mut findings,
          "documents.csv",
          &row.document_id,
          &row.local_path,
          item,
        );
      }
    }
  }
  seen.clear();
  for row in &variables {
    check_text(
      &mut findings,
      "variables.csv",
      &row.variable_id,
      "variable_id",
      &row.variable_id,
      true,
    );
    duplicate(
      &mut findings,
      &mut seen,
      "variables.csv",
      "variable_id",
      &row.variable_id,
    );
    check_text(
      &mut findings,
      "variables.csv",
      &row.variable_id,
      "variable_name",
      &row.variable_name,
      true,
    );
    if !dataset_ids.contains(row.dataset_id.as_str()) {
      findings.push(finding(
        "variables.csv",
        &row.variable_id,
        "dataset_id",
        QaSeverity::Error,
        format!(
          "dataset_id does not exist in datasets metadata: {}",
          row.dataset_id
        ),
      ));
    }
    check_url(
      &mut findings,
      "variables.csv",
      &row.variable_id,
      &row.source_url,
    );
    check_source_document(
      &mut findings,
      "variables.csv",
      &row.variable_id,
      "source_document",
      &row.source_document,
    );
    if !manifest_by_url.contains_key(row.source_url.as_str()) {
      findings.push(finding(
        "variables.csv",
        &row.variable_id,
        "source_url",
        QaSeverity::Error,
        format!(
          "source_url not found in archive manifest: {}",
          row.source_url
        ),
      ));
    }
    check_text(
      &mut findings,
      "variables.csv",
      &row.variable_id,
      "chunk_id",
      &row.chunk_id,
      true,
    );
  }
  seen.clear();
  for row in &canonical {
    check_text(
      &mut findings,
      "canonical_variables.csv",
      &row.variable_id,
      "variable_id",
      &row.variable_id,
      true,
    );
    duplicate(
      &mut findings,
      &mut seen,
      "canonical_variables.csv",
      "variable_id",
      &row.variable_id,
    );
    if row.variable_name.trim().is_empty() {
      findings.push(finding(
        "canonical_variables.csv",
        &row.variable_id,
        "variable_name",
        QaSeverity::Warning,
        "Canonical variable row has empty variable_name",
      ));
    }
    check_url(
      &mut findings,
      "canonical_variables.csv",
      &row.variable_id,
      &row.source_url,
    );
    check_source_document(
      &mut findings,
      "canonical_variables.csv",
      &row.variable_id,
      "source_document",
      &row.source_document,
    );
    if !manifest_by_url.contains_key(row.source_url.as_str()) {
      findings.push(finding(
        "canonical_variables.csv",
        &row.variable_id,
        "source_url",
        QaSeverity::Error,
        format!(
          "source_url not found in archive manifest: {}",
          row.source_url
        ),
      ));
    }
  }
  for (index, row) in document_edges.iter().enumerate() {
    let label = format!("Line {}", index + 2);
    if !dataset_ids.contains(row.source_id.as_str())
      && !document_ids.contains(row.source_id.as_str())
    {
      findings.push(finding(
        "document_edges.csv",
        &label,
        "source_id",
        QaSeverity::Error,
        format!(
          "source_id '{}' does not map to any dataset or document",
          row.source_id
        ),
      ));
    }
    if !dataset_ids.contains(row.target_id.as_str())
      && !document_ids.contains(row.target_id.as_str())
    {
      findings.push(finding(
        "document_edges.csv",
        &label,
        "target_id",
        QaSeverity::Error,
        format!(
          "target_id '{}' does not map to any dataset or document",
          row.target_id
        ),
      ));
    }
    if !row.source_url.is_empty() && !valid_url(&row.source_url) {
      findings.push(finding(
        "document_edges.csv",
        &label,
        "source_url",
        QaSeverity::Warning,
        format!("Invalid source_url: {}", row.source_url),
      ));
    } else if !row.source_url.is_empty() && !manifest_by_url.contains_key(row.source_url.as_str()) {
      findings.push(finding(
        "document_edges.csv",
        &label,
        "source_url",
        QaSeverity::Warning,
        format!(
          "source_url not found in archive manifest: {}",
          row.source_url
        ),
      ));
    }
    check_optional_evidence(
      &mut findings,
      "document_edges.csv",
      &label,
      &row.local_path,
      &row.sha256,
    );
  }
  for (index, row) in variable_edges.iter().enumerate() {
    let label = format!("Line {}", index + 2);
    if !dataset_ids.contains(row.source_id.as_str()) {
      findings.push(finding(
        "variable_edges.csv",
        &label,
        "source_id",
        QaSeverity::Error,
        format!(
          "source_id does not exist in datasets metadata: {}",
          row.source_id
        ),
      ));
    }
    if !variable_ids.contains(row.target_id.as_str()) {
      findings.push(finding(
        "variable_edges.csv",
        &label,
        "target_id",
        QaSeverity::Error,
        format!(
          "target_id does not exist in variables metadata: {}",
          row.target_id
        ),
      ));
    }
    if row.relationship != "contains" {
      findings.push(finding(
        "variable_edges.csv",
        &label,
        "relationship",
        QaSeverity::Warning,
        format!("Unexpected variable relationship: {}", row.relationship),
      ));
    }
    check_url(&mut findings, "variable_edges.csv", &label, &row.source_url);
    if valid_url(&row.source_url) && !manifest_by_url.contains_key(row.source_url.as_str()) {
      findings.push(finding(
        "variable_edges.csv",
        &label,
        "source_url",
        QaSeverity::Error,
        format!(
          "source_url not found in archive manifest: {}",
          row.source_url
        ),
      ));
    }
    check_source_document(
      &mut findings,
      "variable_edges.csv",
      &label,
      "source_document",
      &row.source_document,
    );
    check_text(
      &mut findings,
      "variable_edges.csv",
      &label,
      "chunk_id",
      &row.chunk_id,
      true,
    );
  }
  for (index, row) in data_edges.iter().enumerate() {
    let label = format!("Line {}", index + 2);
    if !dataset_ids.contains(row.source_id.as_str()) {
      findings.push(finding(
        "data_source_variable_edges.csv",
        &label,
        "source_id",
        QaSeverity::Error,
        format!(
          "source_id does not exist in datasets metadata: {}",
          row.source_id
        ),
      ));
    }
    if !canonical_ids.contains(row.target_id.as_str()) {
      findings.push(finding(
        "data_source_variable_edges.csv",
        &label,
        "target_id",
        QaSeverity::Error,
        format!(
          "target_id does not exist in canonical variables metadata: {}",
          row.target_id
        ),
      ));
    }
    if row.relationship != "contains" {
      findings.push(finding(
        "data_source_variable_edges.csv",
        &label,
        "relationship",
        QaSeverity::Warning,
        format!(
          "Unexpected data source variable relationship: {}",
          row.relationship
        ),
      ));
    }
    for (field, value) in [
      ("source_url", row.source_url.as_str()),
      ("variable_url", row.variable_url.as_str()),
    ] {
      if !valid_url(value) {
        findings.push(finding(
          "data_source_variable_edges.csv",
          &label,
          field,
          QaSeverity::Error,
          format!("Invalid {field}: {value}"),
        ));
      } else if !manifest_by_url.contains_key(value) {
        findings.push(finding(
          "data_source_variable_edges.csv",
          &label,
          field,
          QaSeverity::Error,
          format!("{field} not found in archive manifest: {value}"),
        ));
      }
    }
    check_source_document(
      &mut findings,
      "data_source_variable_edges.csv",
      &label,
      "variable_document",
      &row.variable_document,
    );
  }
  seen.clear();
  for row in &nodes {
    check_text(
      &mut findings,
      "ontology_nodes.csv",
      &row.node_id,
      "node_id",
      &row.node_id,
      true,
    );
    if !["Dataset", "Table", "Variable", "Program"].contains(&row.node_class.as_str()) {
      findings.push(finding(
        "ontology_nodes.csv",
        &row.node_id,
        "node_class",
        QaSeverity::Error,
        format!(
          "Invalid node_class '{}' for node {}",
          row.node_class, row.node_id
        ),
      ));
    }
    duplicate(
      &mut findings,
      &mut seen,
      "ontology_nodes.csv",
      "node_id",
      &row.node_id,
    );
    check_text(
      &mut findings,
      "ontology_nodes.csv",
      &row.node_id,
      "node_class",
      &row.node_class,
      true,
    );
    check_url(
      &mut findings,
      "ontology_nodes.csv",
      &row.node_id,
      &row.source_url,
    );
    check_evidence(
      &mut findings,
      "ontology_nodes.csv",
      &row.node_id,
      &row.local_path,
      &row.sha256,
    );
  }
  let valid_ids: HashSet<&str> = dataset_ids
    .iter()
    .chain(document_ids.iter())
    .chain(variable_ids.iter())
    .chain(node_ids.iter())
    .copied()
    .collect();
  for (index, row) in ontology_edges.iter().enumerate() {
    let item = format!("Line {}", index + 2);
    if !valid_ids.contains(row.source_id.as_str()) {
      findings.push(finding(
        "ontology_edges.csv",
        &item,
        "source_id",
        QaSeverity::Error,
        format!(
          "source_id does not exist in ontology nodes: {}",
          row.source_id
        ),
      ));
    }
    if !valid_ids.contains(row.target_id.as_str()) {
      findings.push(finding(
        "ontology_edges.csv",
        &item,
        "target_id",
        QaSeverity::Warning,
        format!(
          "target_id does not exist in ontology nodes: {}",
          row.target_id
        ),
      ));
    }
    check_url(&mut findings, "ontology_edges.csv", &item, &row.source_url);
    check_evidence(
      &mut findings,
      "ontology_edges.csv",
      &item,
      &row.local_path,
      &row.sha256,
    );
  }

  finish(
    config,
    findings,
    datasets.len(),
    documents.len(),
    variables.len() + canonical.len(),
    document_edges.len() + variable_edges.len() + data_edges.len() + ontology_edges.len(),
  )
}
