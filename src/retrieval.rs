//! `SQLite` FTS5 indexing and deterministic lexical retrieval.

use crate::config::RetrievalConfig;
use crate::error::AppError;
use crate::records::ChunkMetadata;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;

const MAX_SEARCH_LIMIT: usize = 1_000;

/// The canonical kind of a flattened retrieval record.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordType {
  Dataset,
  Document,
  Variable,
  Chunk,
}

impl RecordType {
  #[must_use]
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Dataset => "dataset",
      Self::Document => "document",
      Self::Variable => "variable",
      Self::Chunk => "chunk",
    }
  }

  fn parse(value: &str) -> Result<Self, AppError> {
    match value {
      "dataset" => Ok(Self::Dataset),
      "document" => Ok(Self::Document),
      "variable" => Ok(Self::Variable),
      "chunk" => Ok(Self::Chunk),
      _ => Err(retrieval_error(format!("unknown record type: {value}"))),
    }
  }
}

/// A canonical artifact row flattened for indexing.
#[derive(Clone, Debug, PartialEq)]
pub struct RetrievableRecord {
  pub record_id: String,
  pub record_type: RecordType,
  pub title: String,
  pub dataset_id: String,
  pub text: String,
  pub source_url: String,
  pub source_document: String,
  pub page: Option<usize>,
  pub exact_terms: Vec<String>,
}

/// A deterministic citation-bearing lexical search hit.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SearchResult {
  pub record_id: String,
  pub record_type: RecordType,
  pub title: String,
  pub dataset_id: String,
  pub score: f64,
  pub snippet: String,
  pub source_url: String,
  pub source_document: String,
  pub page: Option<usize>,
}

fn retrieval_error(message: impl Into<String>) -> AppError {
  AppError::RetrievalError(message.into())
}

fn read_csv_rows(
  path: &Path,
  required_headers: &[&str],
) -> Result<Vec<HashMap<String, String>>, AppError> {
  let mut reader = csv::Reader::from_path(path)
    .map_err(|error| retrieval_error(format!("failed to read {}: {error}", path.display())))?;
  let headers = reader
    .headers()
    .map_err(|error| retrieval_error(format!("failed to read {} header: {error}", path.display())))?
    .clone();
  let missing = required_headers
    .iter()
    .filter(|required| !headers.iter().any(|header| header == **required))
    .copied()
    .collect::<Vec<_>>();
  if !missing.is_empty() {
    return Err(retrieval_error(format!(
      "{} is missing columns: {}",
      path.display(),
      missing.join(", ")
    )));
  }

  reader
    .records()
    .map(|result| {
      let row = result
        .map_err(|error| retrieval_error(format!("failed to parse {}: {error}", path.display())))?;
      Ok(
        headers
          .iter()
          .zip(row.iter())
          .map(|(header, value)| (header.to_string(), value.to_string()))
          .collect(),
      )
    })
    .collect()
}

fn value(row: &HashMap<String, String>, field: &str) -> String {
  row.get(field).cloned().unwrap_or_default()
}

fn required_value(
  row: &HashMap<String, String>,
  field: &str,
  row_id: &str,
) -> Result<String, AppError> {
  let value = value(row, field).trim().to_string();
  if value.is_empty() {
    Err(retrieval_error(format!(
      "{row_id} has empty required field: {field}"
    )))
  } else {
    Ok(value)
  }
}

fn joined_text(values: &[String]) -> String {
  values
    .iter()
    .filter(|value| !value.is_empty())
    .cloned()
    .collect::<Vec<_>>()
    .join(" ")
}

fn dataset_record(row: &HashMap<String, String>) -> Result<RetrievableRecord, AppError> {
  let dataset_id = required_value(row, "dataset_id", "dataset row")?;
  let title = match value(row, "name") {
    name if name.is_empty() => dataset_id.clone(),
    name => name,
  };
  Ok(RetrievableRecord {
    record_id: dataset_id.clone(),
    record_type: RecordType::Dataset,
    title: title.clone(),
    dataset_id: dataset_id.clone(),
    text: joined_text(&[
      dataset_id.clone(),
      title.clone(),
      value(row, "program"),
      value(row, "category"),
      value(row, "availability"),
      value(row, "extraction_notes"),
    ]),
    source_url: required_value(row, "source_url", &dataset_id)?,
    source_document: value(row, "local_path"),
    page: None,
    exact_terms: vec![dataset_id, title],
  })
}

fn document_record(row: &HashMap<String, String>) -> Result<RetrievableRecord, AppError> {
  let document_id = required_value(row, "document_id", "document row")?;
  let title = match value(row, "title") {
    title if title.is_empty() => document_id.clone(),
    title => title,
  };
  let dataset_id = value(row, "dataset_id");
  Ok(RetrievableRecord {
    record_id: document_id.clone(),
    record_type: RecordType::Document,
    title: title.clone(),
    dataset_id: dataset_id.clone(),
    text: joined_text(&[
      document_id.clone(),
      dataset_id.clone(),
      title.clone(),
      value(row, "document_kind"),
      value(row, "content_type"),
      value(row, "extraction_notes"),
    ]),
    source_url: required_value(row, "source_url", &document_id)?,
    source_document: value(row, "local_path"),
    page: None,
    exact_terms: vec![document_id, dataset_id, title],
  })
}

fn variable_record(row: &HashMap<String, String>) -> Result<RetrievableRecord, AppError> {
  let variable_id = required_value(row, "variable_id", "variable row")?;
  let variable_name = required_value(row, "variable_name", &variable_id)?;
  let dataset_id = value(row, "dataset_id");
  let page = match value(row, "page").trim() {
    "" => None,
    value => Some(
      value
        .parse::<usize>()
        .map_err(|error| retrieval_error(format!("{variable_id} has invalid page: {error}")))?,
    ),
  };
  Ok(RetrievableRecord {
    record_id: variable_id.clone(),
    record_type: RecordType::Variable,
    title: variable_name.clone(),
    dataset_id: dataset_id.clone(),
    text: joined_text(&[
      variable_id.clone(),
      variable_name.clone(),
      dataset_id.clone(),
      value(row, "definition"),
      value(row, "aliases").replace('|', " "),
      value(row, "years").replace('|', " "),
      value(row, "extraction_notes"),
    ]),
    source_url: required_value(row, "source_url", &variable_id)?,
    source_document: value(row, "source_document"),
    page,
    exact_terms: vec![variable_id, variable_name, dataset_id],
  })
}

fn chunk_record(chunk: ChunkMetadata) -> Result<RetrievableRecord, AppError> {
  if chunk.url.trim().is_empty() {
    return Err(retrieval_error(format!(
      "{} has empty required field: url",
      chunk.chunk_id
    )));
  }
  Ok(RetrievableRecord {
    record_id: chunk.chunk_id.clone(),
    record_type: RecordType::Chunk,
    title: chunk.chunk_id.clone(),
    dataset_id: chunk.dataset.clone(),
    text: chunk.text,
    source_url: chunk.url,
    source_document: chunk.source_document,
    page: chunk.page,
    exact_terms: vec![chunk.chunk_id, chunk.dataset],
  })
}

fn load_chunks(path: &Path) -> Result<Vec<RetrievableRecord>, AppError> {
  let file = File::open(path)
    .map_err(|error| retrieval_error(format!("failed to read {}: {error}", path.display())))?;
  BufReader::new(file)
    .lines()
    .enumerate()
    .filter_map(|(index, result)| match result {
      Ok(line) if line.trim().is_empty() => None,
      result => Some((index, result)),
    })
    .map(|(index, result)| {
      let line = result
        .map_err(|error| retrieval_error(format!("failed to read {}: {error}", path.display())))?;
      let chunk = serde_json::from_str::<ChunkMetadata>(&line).map_err(|error| {
        retrieval_error(format!(
          "failed to parse chunk JSON on line {}: {error}",
          index + 1
        ))
      })?;
      chunk_record(chunk)
    })
    .collect()
}

/// Loads required metadata and any present optional retrieval artifacts.
///
/// # Errors
///
/// Returns [`AppError`] when a required file, schema, row, or optional artifact is invalid.
pub fn load_retrievable_records(
  config: &RetrievalConfig,
) -> Result<Vec<RetrievableRecord>, AppError> {
  let mut records = Vec::new();
  for row in read_csv_rows(
    &config.datasets_metadata_path,
    &["dataset_id", "name", "source_url"],
  )? {
    records.push(dataset_record(&row)?);
  }
  for row in read_csv_rows(
    &config.documents_metadata_path,
    &["document_id", "dataset_id", "title", "source_url"],
  )? {
    records.push(document_record(&row)?);
  }
  if config.variables_metadata_path.is_file() {
    for row in read_csv_rows(
      &config.variables_metadata_path,
      &[
        "variable_id",
        "variable_name",
        "dataset_id",
        "definition",
        "source_url",
        "source_document",
        "page",
      ],
    )? {
      records.push(variable_record(&row)?);
    }
  }
  if config.chunks_jsonl_path.is_file() {
    records.extend(load_chunks(&config.chunks_jsonl_path)?);
  }
  Ok(records)
}

fn remove_temp_database(path: &Path) -> Result<(), AppError> {
  if path.exists() {
    fs::remove_file(path)
      .map_err(|error| retrieval_error(format!("failed to remove {}: {error}", path.display())))?;
  }
  Ok(())
}

const EMBEDDING_DIMENSIONS: usize = 384;

fn embedding_for_text(text: &str) -> Vec<f32> {
  let mut embedding = vec![0.0_f32; EMBEDDING_DIMENSIONS];
  for (index, byte) in text.bytes().enumerate() {
    let slot = (usize::from(byte) + index) % EMBEDDING_DIMENSIONS;
    embedding[slot] += f32::from(byte) / 255.0;
  }
  let norm = embedding
    .iter()
    .map(|value| value * value)
    .sum::<f32>()
    .sqrt();
  if norm > 0.0 {
    for value in &mut embedding {
      *value /= norm;
    }
  }
  embedding
}

fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
  embedding
    .iter()
    .flat_map(|value| value.to_le_bytes())
    .collect()
}

fn embedding_from_blob(blob: &[u8]) -> Option<Vec<f32>> {
  if !blob.len().is_multiple_of(std::mem::size_of::<f32>()) {
    return None;
  }
  Some(
    blob
      .chunks_exact(std::mem::size_of::<f32>())
      .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
      .collect(),
  )
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f64 {
  left
    .iter()
    .zip(right.iter())
    .map(|(left, right)| f64::from(*left) * f64::from(*right))
    .sum()
}

fn has_embedding_table(connection: &Connection) -> Result<bool, AppError> {
  connection
    .query_row(
      "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='record_embeddings'",
      [],
      |row| row.get::<_, usize>(0),
    )
    .map(|count| count > 0)
    .map_err(|error| retrieval_error(format!("failed to inspect embedding table: {error}")))
}

fn write_index(
  path: &Path,
  records: &[RetrievableRecord],
  build_embeddings: bool,
) -> Result<(), AppError> {
  let mut connection = Connection::open(path)
    .map_err(|error| retrieval_error(format!("failed to open {}: {error}", path.display())))?;
  connection
    .execute_batch(
      "PRAGMA foreign_keys = ON;
       CREATE TABLE records (
         record_id TEXT PRIMARY KEY,
         record_type TEXT NOT NULL,
         title TEXT NOT NULL,
         dataset_id TEXT NOT NULL,
         source_url TEXT NOT NULL,
         source_document TEXT NOT NULL,
         page INTEGER,
         exact_terms TEXT NOT NULL
       );
       CREATE VIRTUAL TABLE records_fts USING fts5(
         record_id, title, dataset_id, text,
         tokenize=\"unicode61 tokenchars '_'\"
       );",
    )
    .map_err(|error| retrieval_error(format!("failed to initialize index: {error}")))?;
  let transaction = connection
    .transaction()
    .map_err(|error| retrieval_error(format!("failed to start index transaction: {error}")))?;
  if build_embeddings {
    transaction
      .execute(
        "CREATE TABLE record_embeddings (
          record_id TEXT PRIMARY KEY,
          embedding BLOB NOT NULL
        )",
        [],
      )
      .map_err(|error| retrieval_error(format!("failed to initialize embeddings: {error}")))?;
  }
  for record in records {
    let page = record
      .page
      .map(i64::try_from)
      .transpose()
      .map_err(|error| retrieval_error(format!("page does not fit SQLite integer: {error}")))?;
    let exact_terms = serde_json::to_string(&record.exact_terms)
      .map_err(|error| retrieval_error(format!("failed to serialize exact terms: {error}")))?;
    transaction
      .execute(
        "INSERT OR REPLACE INTO records
         (record_id, record_type, title, dataset_id, source_url, source_document, page, exact_terms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
          record.record_id,
          record.record_type.as_str(),
          record.title,
          record.dataset_id,
          record.source_url,
          record.source_document,
          page,
          exact_terms
        ],
      )
      .map_err(|error| retrieval_error(format!("failed to insert record: {error}")))?;
    transaction
      .execute(
        "INSERT OR REPLACE INTO records_fts (record_id, title, dataset_id, text)
         VALUES (?1, ?2, ?3, ?4)",
        params![
          record.record_id,
          record.title,
          record.dataset_id,
          record.text
        ],
      )
      .map_err(|error| retrieval_error(format!("failed to insert FTS record: {error}")))?;
    if build_embeddings {
      let embedding_text = joined_text(&[
        record.record_id.clone(),
        record.title.clone(),
        record.dataset_id.clone(),
        record.text.clone(),
      ]);
      let embedding = embedding_to_blob(&embedding_for_text(&embedding_text));
      transaction
        .execute(
          "INSERT OR REPLACE INTO record_embeddings (record_id, embedding) VALUES (?1, ?2)",
          params![record.record_id, embedding],
        )
        .map_err(|error| retrieval_error(format!("failed to insert embedding: {error}")))?;
    }
  }
  transaction
    .commit()
    .map_err(|error| retrieval_error(format!("failed to commit index: {error}")))
}

/// Atomically builds the `SQLite` FTS5 serving index and returns its record count.
///
/// # Errors
///
/// Returns [`AppError`] when artifacts cannot be loaded or the index cannot be replaced.
pub fn build_index(config: &RetrievalConfig) -> Result<usize, AppError> {
  build_index_with_options(config, false)
}

/// Atomically builds the serving index with optional deterministic embeddings.
///
/// # Errors
///
/// Returns [`AppError`] when artifacts cannot be loaded or the index cannot be replaced.
pub fn build_index_with_options(
  config: &RetrievalConfig,
  build_embeddings: bool,
) -> Result<usize, AppError> {
  let records = load_retrievable_records(config)?;
  let parent = config
    .database_path
    .parent()
    .unwrap_or_else(|| Path::new(""));
  fs::create_dir_all(parent)
    .map_err(|error| retrieval_error(format!("failed to create {}: {error}", parent.display())))?;
  let temp_path = config.database_path.with_extension("sqlite.tmp");
  remove_temp_database(&temp_path)?;
  if let Err(error) = write_index(&temp_path, &records, build_embeddings) {
    let _ = remove_temp_database(&temp_path);
    return Err(error);
  }
  fs::rename(&temp_path, &config.database_path).map_err(|error| {
    let _ = remove_temp_database(&temp_path);
    retrieval_error(format!(
      "failed to replace {}: {error}",
      config.database_path.display()
    ))
  })?;
  Ok(records.len())
}

fn tokens(value: &str) -> Vec<String> {
  let mut tokens = Vec::new();
  let mut current = String::new();
  for character in value.chars().flat_map(char::to_lowercase) {
    if character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_' {
      current.push(character);
    } else if !current.is_empty() {
      tokens.push(std::mem::take(&mut current));
    }
  }
  if !current.is_empty() {
    tokens.push(current);
  }
  tokens
}

fn snippet(text: &str, query_tokens: &[String], max_length: usize) -> String {
  let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
  let characters = cleaned.chars().collect::<Vec<_>>();
  if characters.len() <= max_length {
    return cleaned;
  }
  let lowered = cleaned.to_lowercase();
  let first_match_bytes = query_tokens
    .iter()
    .filter_map(|token| lowered.find(token))
    .min()
    .unwrap_or(0);
  let first_match = lowered[..first_match_bytes].chars().count();
  let start = first_match.saturating_sub(40);
  let end = characters.len().min(start + max_length);
  let mut result = characters[start..end]
    .iter()
    .collect::<String>()
    .trim()
    .to_string();
  if start > 0 {
    result.insert_str(0, "...");
  }
  if end < characters.len() {
    result.push_str("...");
  }
  result
}

fn field_boost(query: &str, query_tokens: &[String], text: &str, exact_terms: &[String]) -> f64 {
  let exact_values = exact_terms
    .iter()
    .filter(|term| !term.is_empty())
    .map(|term| term.to_lowercase())
    .collect::<Vec<_>>();
  let exact_query_boost = if exact_values.iter().any(|value| value == query) {
    8.0
  } else {
    0.0
  };
  let mut unique_tokens = query_tokens.to_vec();
  unique_tokens.sort_unstable();
  unique_tokens.dedup();
  let token_boost = unique_tokens
    .iter()
    .filter(|token| exact_values.contains(token))
    .fold(0.0, |boost, _| boost + 4.0);
  let substring_boost = if text.to_lowercase().contains(query) {
    2.0
  } else {
    0.0
  };
  exact_query_boost + token_boost + substring_boost
}

/// Searches a previously built `SQLite` FTS5 index.
///
/// # Errors
///
/// Returns [`AppError`] for invalid queries, a missing index, or invalid index records.
#[allow(clippy::too_many_lines)]
pub fn search_index(
  database_path: &Path,
  query: &str,
  limit: usize,
) -> Result<Vec<SearchResult>, AppError> {
  search_index_with_options(database_path, query, limit, false, 0.5)
}

/// Searches a previously built index with optional embedding reranking.
///
/// # Errors
///
/// Returns [`AppError`] for invalid queries, a missing index, or invalid index records.
#[allow(clippy::too_many_lines)]
pub fn search_index_with_options(
  database_path: &Path,
  query: &str,
  limit: usize,
  hybrid: bool,
  semantic_weight: f64,
) -> Result<Vec<SearchResult>, AppError> {
  let normalized_query = query.trim().to_lowercase();
  if normalized_query.is_empty() {
    return Err(retrieval_error("query must not be empty"));
  }
  if limit == 0 {
    return Err(retrieval_error("limit must be greater than 0"));
  }
  let query_tokens = tokens(&normalized_query);
  if query_tokens.is_empty() {
    return Err(retrieval_error(
      "query must contain at least one searchable token",
    ));
  }
  if !database_path.is_file() {
    return Err(retrieval_error(format!(
      "Search index not found at {}. Please run index building first.",
      database_path.display()
    )));
  }
  let effective_limit = limit.min(MAX_SEARCH_LIMIT);
  let candidate_limit = 500.max(effective_limit.saturating_mul(5));
  let candidate_limit = i64::try_from(candidate_limit)
    .map_err(|error| retrieval_error(format!("candidate limit is invalid: {error}")))?;
  let match_expression = query_tokens
    .iter()
    .map(|token| format!("\"{token}\""))
    .collect::<Vec<_>>()
    .join(" OR ");
  let connection = Connection::open(database_path).map_err(|error| {
    retrieval_error(format!(
      "failed to open {}: {error}",
      database_path.display()
    ))
  })?;
  let use_embeddings = hybrid && has_embedding_table(&connection)?;
  let query_embedding = if use_embeddings {
    Some(embedding_for_text(&normalized_query))
  } else {
    None
  };
  let mut statement = connection
    .prepare(
      "SELECT r.record_id, r.record_type, r.title, r.dataset_id,
              r.source_url, r.source_document, r.page, r.exact_terms,
              fts.text, -bm25(records_fts, 10.0, 5.0, 2.0, 1.0) AS fts_score
       FROM records r
       JOIN records_fts fts ON r.record_id = fts.record_id
       WHERE records_fts MATCH ?1
       ORDER BY fts_score DESC
       LIMIT ?2",
    )
    .map_err(|error| retrieval_error(format!("failed to prepare search: {error}")))?;
  let rows = statement
    .query_map(params![match_expression, candidate_limit], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, String>(3)?,
        row.get::<_, String>(4)?,
        row.get::<_, String>(5)?,
        row.get::<_, Option<i64>>(6)?,
        row.get::<_, String>(7)?,
        row.get::<_, String>(8)?,
        row.get::<_, f64>(9)?,
      ))
    })
    .map_err(|error| retrieval_error(format!("failed to execute search: {error}")))?;

  let mut results = Vec::new();
  let mut max_lexical_score = 0.0_f64;
  for row in rows {
    let (
      record_id,
      record_type,
      title,
      dataset_id,
      source_url,
      source_document,
      page,
      exact_terms,
      text,
      fts_score,
    ) = row.map_err(|error| retrieval_error(format!("failed to read search result: {error}")))?;
    let exact_terms = serde_json::from_str::<Vec<String>>(&exact_terms)
      .map_err(|error| retrieval_error(format!("invalid exact terms for {record_id}: {error}")))?;
    let page = page
      .map(usize::try_from)
      .transpose()
      .map_err(|error| retrieval_error(format!("invalid page for {record_id}: {error}")))?;
    let lexical_score =
      fts_score + field_boost(&normalized_query, &query_tokens, &text, &exact_terms);
    max_lexical_score = max_lexical_score.max(lexical_score);
    results.push(SearchResult {
      record_id,
      record_type: RecordType::parse(&record_type)?,
      title,
      dataset_id,
      score: lexical_score,
      snippet: snippet(&text, &query_tokens, 180),
      source_url,
      source_document,
      page,
    });
  }
  if let Some(query_embedding) = query_embedding {
    let mut embeddings = HashMap::new();
    let mut statement = connection
      .prepare("SELECT record_id, embedding FROM record_embeddings")
      .map_err(|error| retrieval_error(format!("failed to prepare embedding read: {error}")))?;
    let rows = statement
      .query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
      })
      .map_err(|error| retrieval_error(format!("failed to read embeddings: {error}")))?;
    for row in rows {
      let (record_id, blob) =
        row.map_err(|error| retrieval_error(format!("failed to read embedding row: {error}")))?;
      if let Some(embedding) = embedding_from_blob(&blob) {
        embeddings.insert(record_id, embedding);
      }
    }
    let lexical_denominator = if max_lexical_score > 0.0 {
      max_lexical_score
    } else {
      1.0
    };
    for result in &mut results {
      if let Some(embedding) = embeddings.get(&result.record_id) {
        let lexical = result.score / lexical_denominator;
        let semantic = cosine_similarity(&query_embedding, embedding);
        let exact_guard = if result.title.eq_ignore_ascii_case(query)
          || result.record_id.eq_ignore_ascii_case(query)
        {
          8.0
        } else {
          0.0
        };
        result.score =
          ((1.0 - semantic_weight) * lexical) + (semantic_weight * semantic) + exact_guard;
      }
    }
  }
  for result in &mut results {
    result.score = (result.score * 1_000_000.0).round() / 1_000_000.0;
  }
  results.sort_by(|left, right| {
    right
      .score
      .total_cmp(&left.score)
      .then_with(|| left.record_type.as_str().cmp(right.record_type.as_str()))
      .then_with(|| left.record_id.cmp(&right.record_id))
  });
  results.truncate(effective_limit);
  Ok(results)
}

/// Validates required inputs and runs SQLite-backed lexical retrieval.
///
/// # Errors
///
/// Returns [`AppError`] when required metadata is absent or indexed search fails.
pub fn run_retrieval(
  config: &RetrievalConfig,
  query: &str,
  limit: usize,
) -> Result<Vec<SearchResult>, AppError> {
  if !config.datasets_metadata_path.is_file() {
    return Err(retrieval_error(format!(
      "Datasets metadata file not found at {}",
      config.datasets_metadata_path.display()
    )));
  }
  if !config.documents_metadata_path.is_file() {
    return Err(retrieval_error(format!(
      "Documents metadata file not found at {}",
      config.documents_metadata_path.display()
    )));
  }
  config.validate()?;
  search_index_with_options(
    &config.database_path,
    query,
    limit,
    config.hybrid_search_enabled,
    config.semantic_weight,
  )
}
