//! Metadata extraction from archived CMS ResDAC documents.

#![allow(
  clippy::collapsible_if,
  clippy::if_not_else,
  clippy::missing_errors_doc,
  clippy::missing_panics_doc,
  clippy::too_many_lines,
  clippy::uninlined_format_args,
  clippy::manual_string_new,
  clippy::redundant_closure,
  clippy::implicit_hasher,
  clippy::doc_markdown,
  clippy::too_many_arguments
)]

use crate::config::ExtractionConfig;
use crate::error::AppError;
use crate::inventory;
use crate::records::{
  ArchiveManifestRow, DatasetMetadataRow, DocumentEdgeRow, DocumentMetadataRow, OntologyEdgeRow,
  OntologyNodeRow,
};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha256;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use url::Url;

/// Structured failure logged when a manifest resource fails checksum or presence check.
#[derive(Clone, Debug)]
pub struct ExtractionFailure {
  pub url: String,
  pub resource_kind: String,
  pub local_path: String,
  pub reason: String,
}

fn read_archive_manifest(path: &Path) -> Result<Vec<ArchiveManifestRow>, AppError> {
  let file = File::open(path).map_err(|e| {
    AppError::RecordParseError(format!(
      "failed to open archive manifest at {}: {e}",
      path.display()
    ))
  })?;
  let mut reader = csv::Reader::from_reader(file);
  let mut rows = Vec::new();
  for result in reader.deserialize() {
    let row: ArchiveManifestRow = result.map_err(|e| {
      AppError::RecordParseError(format!("failed to deserialize manifest row: {e}"))
    })?;
    rows.push(row);
  }
  Ok(rows)
}

fn slugify(value: &str) -> String {
  let mut slug = String::new();
  let mut last_was_dash = false;
  for c in value.to_lowercase().chars() {
    if c.is_ascii_alphanumeric() {
      slug.push(c);
      last_was_dash = false;
    } else if !last_was_dash {
      slug.push('-');
      last_was_dash = true;
    }
  }
  let mut trimmed = slug.trim_matches('-').to_string();
  if trimmed.is_empty() {
    trimmed = "unknown".to_string();
  }
  trimmed
}

fn dataset_id_from_url(url_str: &str) -> String {
  if let Some(dataset_id) = dataset_id_from_resdac_file_url(url_str) {
    return dataset_id;
  }
  if let Ok(parsed) = Url::parse(url_str) {
    let path = parsed.path().trim_end_matches('/');
    let path_buf = Path::new(path);
    if let Some(stem) = path_buf.file_stem().and_then(|s| s.to_str()) {
      return slugify(stem);
    }
  }
  "unknown".to_string()
}

fn dataset_id_from_resdac_file_url(url_str: &str) -> Option<String> {
  if let Ok(parsed) = Url::parse(url_str) {
    let path = parsed.path().trim_end_matches('/');
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() >= 3 && parts[0] == "cms-data" && parts[1] == "files" {
      return Some(slugify(parts[2]));
    }
  }
  None
}

fn document_suffix_from_url(url_str: &str) -> String {
  if let Ok(parsed) = Url::parse(url_str) {
    let path = parsed.path().trim_end_matches('/');
    let path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if path.ends_with("/data-documentation") || path_parts.last() == Some(&"data-documentation") {
      return "data-documentation".to_string();
    }
    let path_buf = Path::new(path);
    if let Some(file_name) = path_buf.file_name().and_then(|s| s.to_str()) {
      let file_path = Path::new(file_name);
      let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
      let ext = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");
      let slugified_stem = slugify(stem);
      if !ext.is_empty() {
        return format!("{}_{}", slugified_stem, slugify(ext));
      }
      return slugified_stem;
    }
  }
  "document".to_string()
}

fn stable_url_hash(url: &str) -> String {
  let mut hasher = Sha1::new();
  hasher.update(url.as_bytes());
  let result = hasher.finalize();
  format!("{:x}", result)[..10].to_string()
}

fn compute_file_sha256(path: &Path) -> Result<String, std::io::Error> {
  let mut file = File::open(path)?;
  let mut hasher = Sha256::new();
  let mut buffer = [0; 8192];
  loop {
    let count = file.read(&mut buffer)?;
    if count == 0 {
      break;
    }
    hasher.update(&buffer[..count]);
  }
  Ok(format!("{:x}", hasher.finalize()))
}

fn verify_archived_row(row: &ArchiveManifestRow) -> Option<ExtractionFailure> {
  let Some(local_path_str) = &row.local_path else {
    return Some(ExtractionFailure {
      url: row.url.clone(),
      resource_kind: row.resource_kind.clone(),
      local_path: String::new(),
      reason: "archived row has no local path".to_string(),
    });
  };

  let local_path = Path::new(local_path_str);
  if !local_path.exists() {
    return Some(ExtractionFailure {
      url: row.url.clone(),
      resource_kind: row.resource_kind.clone(),
      local_path: local_path_str.clone(),
      reason: "archived file does not exist".to_string(),
    });
  }

  let Some(expected_sha256) = &row.sha256 else {
    return Some(ExtractionFailure {
      url: row.url.clone(),
      resource_kind: row.resource_kind.clone(),
      local_path: local_path_str.clone(),
      reason: "archived row has no sha256".to_string(),
    });
  };

  match compute_file_sha256(local_path) {
    Ok(actual_sha256) => {
      if actual_sha256 != *expected_sha256 {
        Some(ExtractionFailure {
          url: row.url.clone(),
          resource_kind: row.resource_kind.clone(),
          local_path: local_path_str.clone(),
          reason: "checksum mismatch".to_string(),
        })
      } else {
        None
      }
    }
    Err(err) => Some(ExtractionFailure {
      url: row.url.clone(),
      resource_kind: row.resource_kind.clone(),
      local_path: local_path_str.clone(),
      reason: format!("error hashing file: {err}"),
    }),
  }
}

fn is_eligible_row(row: &ArchiveManifestRow) -> bool {
  row.archive_state == "archived"
    && (row.resource_kind == "dataset_page"
      || row.resource_kind == "documentation_page"
      || row.resource_kind == "asset")
}

fn is_html_content_type(ct: Option<&str>) -> bool {
  ct.is_some_and(|s| s.to_lowercase().starts_with("text/html"))
}

fn read_html_title(local_path: &Path) -> String {
  if let Ok(bytes) = std::fs::read(local_path) {
    let html = String::from_utf8_lossy(&bytes);
    let (title, _) = inventory::parse_page(&html);
    title
  } else {
    String::new()
  }
}

fn document_kind(row: &ArchiveManifestRow) -> String {
  if row.resource_kind == "documentation_page" {
    return "html".to_string();
  }
  row
    .asset_kind
    .clone()
    .unwrap_or_else(|| "other".to_string())
}

fn extract_document(row: &ArchiveManifestRow, dataset_id: &str) -> DocumentMetadataRow {
  let local_path_str = row.local_path.clone().unwrap_or_default();
  let local_path = Path::new(&local_path_str);

  let title = if row.resource_kind == "documentation_page"
    && is_html_content_type(row.content_type.as_deref())
  {
    read_html_title(local_path)
  } else {
    row.source_title.clone().unwrap_or_default()
  };

  let document_id = format!(
    "{}__{}__{}",
    dataset_id,
    document_suffix_from_url(&row.url),
    stable_url_hash(&row.url)
  );

  DocumentMetadataRow {
    document_id,
    dataset_id: dataset_id.to_string(),
    title,
    document_kind: document_kind(row),
    source_url: row.url.clone(),
    local_path: local_path_str,
    sha256: row.sha256.clone().unwrap_or_default(),
    content_type: row.content_type.clone(),
    extraction_notes: None,
  }
}

fn dataset_id_for_document(row: &ArchiveManifestRow) -> Option<String> {
  if row.resource_kind == "documentation_page" {
    return dataset_id_from_resdac_file_url(&row.url);
  }
  if let Some(source_url) = &row.source_url {
    if let Some(id) = dataset_id_from_resdac_file_url(source_url) {
      return Some(id);
    }
  }
  dataset_id_from_resdac_file_url(&row.url)
}

fn dataset_ids_for_document(
  row: &ArchiveManifestRow,
  linked_asset_dataset_ids: &HashMap<String, HashSet<String>>,
) -> Vec<String> {
  let mut ids = HashSet::new();
  if let Some(dataset_id) = dataset_id_for_document(row) {
    ids.insert(dataset_id);
  }
  if row.resource_kind == "asset" {
    if let Some(ids_set) = linked_asset_dataset_ids.get(&row.url) {
      for id in ids_set {
        ids.insert(id.clone());
      }
    }
  }
  let mut sorted_ids: Vec<String> = ids.into_iter().collect();
  sorted_ids.sort();
  sorted_ids
}

fn edge_for_document(row: &DocumentMetadataRow) -> DocumentEdgeRow {
  DocumentEdgeRow {
    source_id: row.dataset_id.clone(),
    target_id: row.document_id.clone(),
    relationship: "has_document".to_string(),
    source_url: row.source_url.clone(),
    local_path: row.local_path.clone(),
    sha256: row.sha256.clone(),
  }
}

fn write_model_csv<T: serde::Serialize>(rows: &[T], output_path: &Path) -> Result<(), AppError> {
  if let Some(parent) = output_path.parent() {
    std::fs::create_dir_all(parent).map_err(|e| {
      AppError::RecordParseError(format!(
        "failed to create directories for {}: {e}",
        output_path.display()
      ))
    })?;
  }
  let file = File::create(output_path).map_err(|e| {
    AppError::RecordParseError(format!(
      "failed to create file {}: {e}",
      output_path.display()
    ))
  })?;
  let mut writer = csv::WriterBuilder::new()
    .has_headers(true)
    .from_writer(file);
  for row in rows {
    writer.serialize(row).map_err(|e| {
      AppError::RecordParseError(format!(
        "failed to serialize row for {}: {e}",
        output_path.display()
      ))
    })?;
  }
  writer.flush().map_err(|e| {
    AppError::RecordParseError(format!(
      "failed to flush CSV writer for {}: {e}",
      output_path.display()
    ))
  })?;
  Ok(())
}

fn write_extraction_workspace_summary(
  config: &ExtractionConfig,
  manifest_rows: usize,
  datasets: &[DatasetMetadataRow],
  documents: &[DocumentMetadataRow],
  document_edges: &[DocumentEdgeRow],
  ontology_nodes: &[OntologyNodeRow],
  ontology_edges: &[OntologyEdgeRow],
  failures: &[ExtractionFailure],
) -> Result<(), AppError> {
  std::fs::create_dir_all(&config.workspace_dir)
    .map_err(|e| AppError::RecordParseError(format!("failed to create workspace dir: {e}")))?;
  let summary_path = config.workspace_dir.join("04_extraction_pack.md");
  let mut lines = vec![
    "# Extraction Pack".to_string(),
    "".to_string(),
    format!(
      "- Archive manifest input: {}",
      config.archive_manifest_path.display()
    ),
    format!("- Manifest rows: {}", manifest_rows),
    format!("- Datasets: {}", datasets.len()),
    format!("- Documents: {}", documents.len()),
    format!("- Document edges: {}", document_edges.len()),
    format!("- Ontology nodes: {}", ontology_nodes.len()),
    format!("- Ontology edges: {}", ontology_edges.len()),
    format!("- Failures: {}", failures.len()),
    "".to_string(),
    "## Outputs".to_string(),
    "".to_string(),
    format!(
      "- Dataset metadata: {}",
      config.metadata_dir.join("datasets.csv").display()
    ),
    format!(
      "- Document metadata: {}",
      config.metadata_dir.join("documents.csv").display()
    ),
    format!(
      "- Document graph edges: {}",
      config.graph_dir.join("document_edges.csv").display()
    ),
    format!(
      "- Ontology graph nodes: {}",
      config.graph_dir.join("ontology_nodes.csv").display()
    ),
    format!(
      "- Ontology graph edges: {}",
      config.graph_dir.join("ontology_edges.csv").display()
    ),
    "".to_string(),
    "## Unresolved Normalization".to_string(),
    "".to_string(),
  ];

  let mut notes = std::collections::BTreeSet::new();
  for row in datasets {
    if let Some(ref note) = row.extraction_notes {
      if !note.is_empty() {
        notes.insert(format!("- {}: {}", row.dataset_id, note));
      }
    }
  }

  if !notes.is_empty() {
    for note in notes {
      lines.push(note);
    }
  } else {
    lines.push("- None".to_string());
  }

  lines.push("".to_string());
  lines.push("## Failures".to_string());
  lines.push("".to_string());

  if !failures.is_empty() {
    lines.push("| url | kind | local_path | reason |".to_string());
    lines.push("| --- | --- | --- | --- |".to_string());
    for failure in failures.iter().take(25) {
      lines.push(format!(
        "| {} | {} | {} | {} |",
        failure.url, failure.resource_kind, failure.local_path, failure.reason
      ));
    }
    if failures.len() > 25 {
      lines.push(format!(
        "\n- Additional failures omitted: {}",
        failures.len() - 25
      ));
    }
  } else {
    lines.push("- None".to_string());
  }

  std::fs::write(&summary_path, lines.join("\n") + "\n")
    .map_err(|e| AppError::RecordParseError(format!("failed to write summary file: {e}")))?;

  Ok(())
}

/// Runs Phase 2 metadata extraction from raw archived files.
pub fn run_extraction(config: &ExtractionConfig) -> Result<(), AppError> {
  let manifest_rows = read_archive_manifest(&config.archive_manifest_path)?;

  let mut datasets_by_id = HashMap::new();
  let mut ontology_nodes = Vec::new();
  let mut ontology_edges = Vec::new();
  let mut failures = Vec::new();
  let mut linked_asset_dataset_ids: HashMap<String, HashSet<String>> = HashMap::new();

  let eligible_rows: Vec<&ArchiveManifestRow> = manifest_rows
    .iter()
    .filter(|r| is_eligible_row(r))
    .collect();

  // Compile selectors outside the loop for performance
  let program_selector =
    scraper::Selector::parse(".views-field-field-program-type .field-content").unwrap();
  let category_selector =
    scraper::Selector::parse(".views-field-field-data-file-category .field-content").unwrap();
  let availability_selector =
    scraper::Selector::parse(".views-field-field-availability .field-content").unwrap();

  // First pass: process datasets to populate asset mapping
  for row in &eligible_rows {
    if row.resource_kind != "dataset_page" {
      continue;
    }
    if let Some(failure) = verify_archived_row(row) {
      failures.push(failure);
      continue;
    }

    let dataset_id = dataset_id_from_url(&row.url);
    let local_path_str = row.local_path.clone().unwrap_or_default();
    let local_path = Path::new(&local_path_str);

    let mut name = row.source_title.clone().unwrap_or_default();
    let mut program = String::new();
    let mut category = String::new();
    let mut availability = String::new();
    let mut links = Vec::new();

    if is_html_content_type(row.content_type.as_deref()) {
      if let Ok(bytes) = std::fs::read(local_path) {
        let html_content = String::from_utf8_lossy(&bytes);
        let (title, parsed_links) = inventory::parse_page(&html_content);

        // Normalize title matching Python
        let mut clean_title = title;
        if let Some(stripped) = clean_title.strip_suffix(" | ResDAC") {
          clean_title = stripped.to_string();
        }
        if !clean_title.is_empty() {
          name = clean_title;
        }

        // Extracted field selectors using scraper
        let doc = scraper::Html::parse_document(&html_content);

        program = doc
          .select(&program_selector)
          .next()
          .map(|el| el.text().collect::<Vec<_>>().join(""))
          .map(|s| inventory::normalize_whitespace(&s))
          .unwrap_or_default();

        category = doc
          .select(&category_selector)
          .next()
          .map(|el| el.text().collect::<Vec<_>>().join(""))
          .map(|s| inventory::normalize_whitespace(&s))
          .unwrap_or_default();

        availability = doc
          .select(&availability_selector)
          .next()
          .map(|el| el.text().collect::<Vec<_>>().join(""))
          .map(|s| inventory::normalize_whitespace(&s))
          .unwrap_or_default();

        for parsed_link in parsed_links {
          links.push(parsed_link.href);
        }
      }
    }

    let dataset = DatasetMetadataRow {
      dataset_id: dataset_id.clone(),
      name: name.clone(),
      program: program.clone(),
      category: category.clone(),
      availability: availability.clone(),
      source_url: row.url.clone(),
      local_path: local_path_str.clone(),
      sha256: row.sha256.clone().unwrap_or_default(),
      extraction_notes: None,
    };

    datasets_by_id.insert(dataset_id.clone(), dataset);

    ontology_nodes.push(OntologyNodeRow {
      node_id: dataset_id.clone(),
      node_class: "Dataset".to_string(),
      name: name.clone(),
      source_url: row.url.clone(),
      local_path: local_path_str.clone(),
      sha256: row.sha256.clone().unwrap_or_default(),
    });

    if !program.is_empty() {
      let program_id = format!("program_{}", slugify(&program));
      ontology_nodes.push(OntologyNodeRow {
        node_id: program_id.clone(),
        node_class: "Program".to_string(),
        name: program.clone(),
        source_url: row.url.clone(),
        local_path: local_path_str.clone(),
        sha256: row.sha256.clone().unwrap_or_default(),
      });
      ontology_edges.push(OntologyEdgeRow {
        source_id: dataset_id.clone(),
        target_id: program_id,
        relationship: "belongs_to".to_string(),
        source_url: row.url.clone(),
        local_path: local_path_str.clone(),
        sha256: row.sha256.clone().unwrap_or_default(),
      });
    }

    let mut seen_related = HashSet::new();
    for href in &links {
      let target_url = inventory::normalize_url(&row.url, href);
      if let Some(target_id) = dataset_id_from_resdac_file_url(&target_url) {
        if target_id != dataset_id && !seen_related.contains(&target_id) {
          seen_related.insert(target_id.clone());
          ontology_edges.push(OntologyEdgeRow {
            source_id: dataset_id.clone(),
            target_id,
            relationship: "related_to".to_string(),
            source_url: row.url.clone(),
            local_path: local_path_str.clone(),
            sha256: row.sha256.clone().unwrap_or_default(),
          });
        }
      }
      linked_asset_dataset_ids
        .entry(target_url)
        .or_default()
        .insert(dataset_id.clone());
    }
  }

  // Second pass: process documents and assets
  let mut seen_documents = HashSet::new();
  let mut documents = Vec::new();

  for row in &eligible_rows {
    if row.resource_kind != "documentation_page" && row.resource_kind != "asset" {
      continue;
    }
    if let Some(failure) = verify_archived_row(row) {
      failures.push(failure);
      continue;
    }

    let dataset_ids = dataset_ids_for_document(row, &linked_asset_dataset_ids);
    if dataset_ids.is_empty() {
      failures.push(ExtractionFailure {
        url: row.url.clone(),
        resource_kind: row.resource_kind.clone(),
        local_path: row.local_path.clone().unwrap_or_default(),
        reason: "document source is not linked to a dataset page".to_string(),
      });
      continue;
    }

    for dataset_id in dataset_ids {
      if !datasets_by_id.contains_key(&dataset_id) {
        failures.push(ExtractionFailure {
          url: row.url.clone(),
          resource_kind: row.resource_kind.clone(),
          local_path: row.local_path.clone().unwrap_or_default(),
          reason: "document references missing dataset".to_string(),
        });
        continue;
      }

      let doc = extract_document(row, &dataset_id);
      if !seen_documents.contains(&doc.document_id) {
        seen_documents.insert(doc.document_id.clone());
        documents.push(doc);
      }
    }
  }

  // Deduplicate and sort ontology nodes by node_id
  let mut unique_nodes_map = HashMap::new();
  for node in ontology_nodes {
    unique_nodes_map.insert(node.node_id.clone(), node);
  }
  let mut unique_nodes: Vec<OntologyNodeRow> = unique_nodes_map.into_values().collect();
  unique_nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));

  // Sort datasets by dataset_id
  let mut sorted_datasets: Vec<DatasetMetadataRow> = datasets_by_id.into_values().collect();
  sorted_datasets.sort_by(|a, b| a.dataset_id.cmp(&b.dataset_id));

  // Sort documents by document_id for determinism
  documents.sort_by(|a, b| a.document_id.cmp(&b.document_id));

  let mut document_edges: Vec<DocumentEdgeRow> = documents.iter().map(edge_for_document).collect();
  // Sort document_edges by source_id, then target_id for determinism
  document_edges.sort_by(|a, b| {
    a.source_id
      .cmp(&b.source_id)
      .then_with(|| a.target_id.cmp(&b.target_id))
  });

  // Deduplicate and sort ontology edges
  let mut unique_edges_map = HashMap::new();
  for edge in ontology_edges {
    let key = (
      edge.source_id.clone(),
      edge.target_id.clone(),
      edge.relationship.clone(),
    );
    unique_edges_map.entry(key).or_insert(edge);
  }
  let mut sorted_ontology_edges: Vec<OntologyEdgeRow> = unique_edges_map.into_values().collect();
  sorted_ontology_edges.sort_by(|a, b| {
    a.source_id
      .cmp(&b.source_id)
      .then_with(|| a.target_id.cmp(&b.target_id))
      .then_with(|| a.relationship.cmp(&b.relationship))
  });

  // Write files
  write_model_csv(&sorted_datasets, &config.metadata_dir.join("datasets.csv"))?;
  write_model_csv(&documents, &config.metadata_dir.join("documents.csv"))?;
  write_model_csv(
    &document_edges,
    &config.graph_dir.join("document_edges.csv"),
  )?;
  write_model_csv(&unique_nodes, &config.graph_dir.join("ontology_nodes.csv"))?;
  write_model_csv(
    &sorted_ontology_edges,
    &config.graph_dir.join("ontology_edges.csv"),
  )?;

  // Write summary markdown pack
  write_extraction_workspace_summary(
    config,
    manifest_rows.len(),
    &sorted_datasets,
    &documents,
    &document_edges,
    &unique_nodes,
    &sorted_ontology_edges,
    &failures,
  )?;

  if !failures.is_empty() {
    return Err(AppError::RecordParseError(format!(
      "extraction completed with {} failures",
      failures.len()
    )));
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_slugify() {
    assert_eq!(
      slugify("Accountable Health Communities Model"),
      "accountable-health-communities-model"
    );
    assert_eq!(slugify("BENE_ID"), "bene-id");
    assert_eq!(slugify("test-run-123"), "test-run-123");
    assert_eq!(slugify(""), "unknown");
  }

  #[test]
  fn test_dataset_id_extraction() {
    assert_eq!(
      dataset_id_from_url("https://resdac.org/cms-data/files/ahc-model"),
      "ahc-model"
    );
    assert_eq!(
      dataset_id_from_url("https://resdac.org/cms-data/files/hha-ffs"),
      "hha-ffs"
    );
    assert_eq!(
      dataset_id_from_url("https://resdac.org/other-path/some-file.html"),
      "some-file"
    );
  }

  #[test]
  fn test_document_suffix() {
    assert_eq!(
      document_suffix_from_url(
        "https://cms.gov/Regulations-and-Guidance/Guidance/Manuals/Downloads/bp102c07.pdf"
      ),
      "bp102c07_pdf"
    );
    assert_eq!(
      document_suffix_from_url("https://resdac.org/cms-data/files/hha-ffs/data-documentation"),
      "data-documentation"
    );
  }

  #[test]
  fn test_stable_url_hash() {
    assert_eq!(
      stable_url_hash(
        "https://cms.gov/Regulations-and-Guidance/Guidance/Manuals/Downloads/bp102c07.pdf"
      ),
      "551d41d9a2"
    );
  }
}
