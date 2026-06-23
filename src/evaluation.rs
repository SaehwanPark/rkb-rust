//! Deterministic retrieval usefulness evaluation.

use crate::agent_context::build_agent_context;
use crate::config::VariableEvaluationConfig;
use crate::error::AppError;
use crate::records::VariableMetadataRow;
use crate::retrieval::{RecordType, SearchResult, run_retrieval};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn evaluation_error(message: impl Into<String>) -> AppError {
  AppError::RetrievalError(message.into())
}

#[allow(clippy::cast_precision_loss)]
fn ratio(numerator: usize, denominator: usize) -> f64 {
  numerator as f64 / denominator as f64
}

#[allow(clippy::cast_precision_loss)]
fn reciprocal(index: usize) -> f64 {
  1.0 / (index + 1) as f64
}

fn read_variable_rows(path: &Path) -> Result<Vec<VariableMetadataRow>, AppError> {
  let mut reader = csv::Reader::from_path(path)
    .map_err(|error| evaluation_error(format!("failed to read {}: {error}", path.display())))?;
  reader
    .deserialize::<VariableMetadataRow>()
    .map(|row| {
      row.map_err(|error| evaluation_error(format!("failed to parse {}: {error}", path.display())))
    })
    .collect()
}

fn deterministic_hash(seed: u64, value: &str) -> u64 {
  let mut hash = seed ^ 0xcbf2_9ce4_8422_2325;
  for byte in value.bytes() {
    hash ^= u64::from(byte);
    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
  }
  hash
}

fn sample_variable_names(
  rows: &[VariableMetadataRow],
  sample_size: usize,
  seed: u64,
) -> Result<Vec<String>, AppError> {
  if sample_size == 0 {
    return Err(AppError::ConfigValidationError(
      "sample_size must be greater than 0".to_string(),
    ));
  }
  let names = rows
    .iter()
    .map(|row| row.variable_name.trim())
    .filter(|name| !name.is_empty())
    .collect::<BTreeSet<_>>();
  if sample_size >= names.len() {
    return Ok(names.into_iter().map(ToOwned::to_owned).collect());
  }
  let mut ranked = names
    .into_iter()
    .map(|name| (deterministic_hash(seed, name), name.to_string()))
    .collect::<Vec<_>>();
  ranked.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
  let mut sampled = ranked
    .into_iter()
    .take(sample_size)
    .map(|(_, name)| name)
    .collect::<Vec<_>>();
  sampled.sort();
  Ok(sampled)
}

fn snippet_has_definition_evidence(result: &SearchResult) -> bool {
  let excluded = [&result.record_id, &result.title, &result.dataset_id]
    .into_iter()
    .filter(|token| !token.is_empty())
    .map(|token| token.to_ascii_lowercase())
    .collect::<BTreeSet<_>>();
  let meaningful_count = result
    .snippet
    .replace(['_', '-'], " ")
    .split_whitespace()
    .filter_map(|word| {
      let trimmed = word.trim_matches([' ', '.', ',', ':', ';', '|', '(', ')']);
      (!trimmed.is_empty()).then(|| trimmed.to_ascii_lowercase())
    })
    .filter(|word| !excluded.contains(word))
    .filter(|word| word.len() > 2 && !word.chars().all(|character| character.is_ascii_digit()))
    .count();
  meaningful_count >= 3
}

fn is_html_result(result: &SearchResult) -> bool {
  let source_document = PathBuf::from(&result.source_document);
  let source_url = result.source_url.to_ascii_lowercase();
  source_document
    .extension()
    .is_some_and(|extension| extension.eq_ignore_ascii_case("html"))
    || source_url.contains("/data-documentation")
}

fn html_evidence_available(rows: &[VariableMetadataRow]) -> bool {
  rows.iter().any(|row| {
    Path::new(&row.source_document)
      .extension()
      .is_some_and(|extension| extension.eq_ignore_ascii_case("html"))
      || row
        .source_url
        .to_ascii_lowercase()
        .contains("/data-documentation")
  })
}

fn evaluate_variable_name(
  config: &VariableEvaluationConfig,
  variable_name: &str,
  rows: &[VariableMetadataRow],
) -> Result<VariableEvaluationCase, AppError> {
  let expected_variable_ids = rows
    .iter()
    .map(|row| row.variable_id.clone())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect::<Vec<_>>();
  let expected_dataset_ids = rows
    .iter()
    .map(|row| row.dataset_id.clone())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect::<Vec<_>>();
  let html_available = html_evidence_available(rows);
  let results = run_retrieval(&config.retrieval, variable_name, config.limit)?;
  let first_matching = results
    .iter()
    .enumerate()
    .find(|(_, result)| {
      result.record_type == RecordType::Variable
        && expected_variable_ids.contains(&result.record_id)
    })
    .map(|(index, result)| (index + 1, result.clone()));
  let (first_matching_rank, first_matching_result) =
    first_matching.map_or((None, None), |(rank, result)| (Some(rank), Some(result)));
  let citation_present = first_matching_result
    .as_ref()
    .is_some_and(|result| !result.source_url.trim().is_empty());
  let snippet_has_definition_evidence = first_matching_result
    .as_ref()
    .is_some_and(snippet_has_definition_evidence);
  let html_preferred_when_available =
    !html_available || first_matching_result.as_ref().is_some_and(is_html_result);
  let passed = first_matching_rank.is_some_and(|rank| rank <= config.limit)
    && citation_present
    && snippet_has_definition_evidence
    && html_preferred_when_available;

  Ok(VariableEvaluationCase {
    variable_name: variable_name.to_string(),
    expected_variable_ids,
    expected_dataset_ids,
    top_result: results.first().cloned(),
    first_matching_rank,
    first_matching_result,
    snippet_has_definition_evidence,
    citation_present,
    html_preferred_when_available,
    html_evidence_available: html_available,
    passed,
  })
}

/// Results of evaluating retrieval for one variable name.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VariableEvaluationCase {
  pub variable_name: String,
  pub expected_variable_ids: Vec<String>,
  pub expected_dataset_ids: Vec<String>,
  pub top_result: Option<SearchResult>,
  pub first_matching_rank: Option<usize>,
  pub first_matching_result: Option<SearchResult>,
  pub snippet_has_definition_evidence: bool,
  pub citation_present: bool,
  pub html_preferred_when_available: bool,
  pub html_evidence_available: bool,
  pub passed: bool,
}

/// Report for a complete variable retrieval evaluation run.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VariableEvaluationReport {
  pub sample_size: usize,
  pub seed: u64,
  pub limit: usize,
  pub cases: Vec<VariableEvaluationCase>,
}

impl VariableEvaluationReport {
  /// Counts cases that passed all checks.
  #[must_use]
  pub fn passed_count(&self) -> usize {
    self.cases.iter().filter(|case| case.passed).count()
  }

  /// Returns the passing fraction across evaluated cases.
  #[must_use]
  pub fn pass_rate(&self) -> f64 {
    if self.cases.is_empty() {
      0.0
    } else {
      ratio(self.passed_count(), self.cases.len())
    }
  }
}

#[derive(Serialize)]
struct VariableEvaluationJson<'a> {
  sample_size: usize,
  seed: u64,
  limit: usize,
  cases: &'a [VariableEvaluationCase],
  passed_count: usize,
  pass_rate: f64,
}

/// Serializes the variable evaluation report with computed summary fields.
///
/// # Errors
///
/// Returns an error if JSON serialization fails.
pub fn variable_report_to_json(report: &VariableEvaluationReport) -> Result<String, AppError> {
  let payload = VariableEvaluationJson {
    sample_size: report.sample_size,
    seed: report.seed,
    limit: report.limit,
    cases: &report.cases,
    passed_count: report.passed_count(),
    pass_rate: report.pass_rate(),
  };
  serde_json::to_string_pretty(&payload).map_err(|error| evaluation_error(error.to_string()))
}

/// Runs seeded exact variable-name retrieval evaluation.
///
/// # Errors
///
/// Returns an error when config validation, CSV loading, or retrieval fails.
pub fn evaluate_variable_retrieval(
  config: &VariableEvaluationConfig,
) -> Result<VariableEvaluationReport, AppError> {
  config.validate()?;
  let rows = read_variable_rows(&config.retrieval.variables_metadata_path)?;
  let mut rows_by_name: BTreeMap<String, Vec<VariableMetadataRow>> = BTreeMap::new();
  for row in &rows {
    rows_by_name
      .entry(row.variable_name.trim().to_string())
      .or_default()
      .push(row.clone());
  }

  let sampled_names = sample_variable_names(&rows, config.sample_size, config.seed)?;
  let cases = sampled_names
    .iter()
    .map(|name| {
      let rows = rows_by_name
        .get(name)
        .ok_or_else(|| evaluation_error(format!("sampled variable name has no rows: {name}")))?;
      evaluate_variable_name(config, name, rows)
    })
    .collect::<Result<Vec<_>, _>>()?;
  Ok(VariableEvaluationReport {
    sample_size: cases.len(),
    seed: config.seed,
    limit: config.limit,
    cases,
  })
}

/// A benchmark query with expected retrieval evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BenchmarkQuestion {
  pub question_id: String,
  pub query: String,
  #[serde(default)]
  pub expected_datasets: Vec<String>,
  #[serde(default)]
  pub expected_variables: Vec<String>,
  #[serde(default)]
  pub expected_documents: Vec<String>,
  #[serde(default)]
  pub expected_citations: Vec<String>,
  #[serde(default)]
  pub description: String,
}

/// A benchmark question collection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BenchmarkQuestionSuite {
  pub questions: Vec<BenchmarkQuestion>,
}

/// Evaluation metrics for one search path.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PathEvaluationResult {
  pub dataset_recall_at_5: f64,
  pub variable_recall_at_5: f64,
  pub citation_accuracy: f64,
  pub dataset_mrr: f64,
  pub variable_mrr: f64,
}

/// Path comparison for one benchmark question.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct QuestionEvaluationResult {
  pub question_id: String,
  pub query: String,
  pub lexical: PathEvaluationResult,
  pub hybrid: PathEvaluationResult,
  pub agent_facing: PathEvaluationResult,
}

/// Benchmark report across all questions.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BenchmarkReport {
  pub mean_lexical: PathEvaluationResult,
  pub mean_hybrid: PathEvaluationResult,
  pub mean_agent_facing: PathEvaluationResult,
  pub results: Vec<QuestionEvaluationResult>,
}

/// Calculates recall at a bounded rank.
#[must_use]
pub fn recall_at_k(retrieved: &[&str], expected: &[&str], k: usize) -> f64 {
  if expected.is_empty() {
    return 1.0;
  }
  let matched = expected
    .iter()
    .filter(|target| retrieved.iter().take(k).any(|item| item == *target))
    .count();
  ratio(matched, expected.len())
}

fn recall_sets_at_k(retrieved: &[BTreeSet<String>], expected: &[String], k: usize) -> f64 {
  if expected.is_empty() {
    return 1.0;
  }
  let matched = expected
    .iter()
    .filter(|target| retrieved.iter().take(k).any(|item| item.contains(*target)))
    .count();
  ratio(matched, expected.len())
}

/// Calculates reciprocal rank.
#[must_use]
pub fn reciprocal_rank(retrieved: &[&str], expected: &[&str]) -> f64 {
  if expected.is_empty() {
    return 1.0;
  }
  retrieved
    .iter()
    .position(|item| expected.iter().any(|target| item == target))
    .map_or(0.0, reciprocal)
}

fn reciprocal_rank_sets(retrieved: &[BTreeSet<String>], expected: &[String]) -> f64 {
  if expected.is_empty() {
    return 1.0;
  }
  retrieved
    .iter()
    .position(|item| expected.iter().any(|target| item.contains(target)))
    .map_or(0.0, reciprocal)
}

/// Calculates the fraction of expected citations present in retrieved citations.
#[must_use]
pub fn citation_accuracy(retrieved: &[String], expected: &[String]) -> f64 {
  if expected.is_empty() {
    return 1.0;
  }
  let normalize = |url: &str| url.trim().trim_end_matches('/').to_ascii_lowercase();
  let retrieved = retrieved
    .iter()
    .filter(|url| !url.is_empty())
    .map(|url| normalize(url))
    .collect::<BTreeSet<_>>();
  let expected = expected
    .iter()
    .filter(|url| !url.is_empty())
    .map(|url| normalize(url))
    .collect::<BTreeSet<_>>();
  if expected.is_empty() {
    return 1.0;
  }
  let matched = expected
    .iter()
    .filter(|url| retrieved.contains(*url))
    .count();
  ratio(matched, expected.len())
}

fn extract_search_ids(
  results: &[SearchResult],
) -> (Vec<String>, Vec<BTreeSet<String>>, Vec<String>) {
  let mut datasets = Vec::new();
  let mut variables = Vec::new();
  let mut citations = Vec::new();
  for result in results {
    if result.record_type == RecordType::Dataset {
      datasets.push(result.record_id.clone());
    } else if !result.dataset_id.is_empty() {
      datasets.push(result.dataset_id.clone());
    }
    if result.record_type == RecordType::Variable {
      variables.push(BTreeSet::from([
        result.record_id.clone(),
        result.title.clone(),
      ]));
    }
    if !result.source_url.is_empty() {
      citations.push(result.source_url.clone());
    }
  }
  (datasets, variables, citations)
}

fn evaluate_path(
  retrieved_datasets: &[String],
  retrieved_variables: &[BTreeSet<String>],
  retrieved_citations: &[String],
  question: &BenchmarkQuestion,
) -> PathEvaluationResult {
  let dataset_refs = retrieved_datasets
    .iter()
    .map(String::as_str)
    .collect::<Vec<_>>();
  let expected_dataset_refs = question
    .expected_datasets
    .iter()
    .map(String::as_str)
    .collect::<Vec<_>>();
  PathEvaluationResult {
    dataset_recall_at_5: recall_at_k(&dataset_refs, &expected_dataset_refs, 5),
    variable_recall_at_5: recall_sets_at_k(retrieved_variables, &question.expected_variables, 5),
    citation_accuracy: citation_accuracy(retrieved_citations, &question.expected_citations),
    dataset_mrr: reciprocal_rank(&dataset_refs, &expected_dataset_refs),
    variable_mrr: reciprocal_rank_sets(retrieved_variables, &question.expected_variables),
  }
}

fn extract_agent_ids(
  context: &crate::agent_context::AgentContext,
) -> (Vec<String>, Vec<BTreeSet<String>>, Vec<String>) {
  let mut datasets = Vec::new();
  let mut variables = Vec::new();
  let mut citations = Vec::new();
  for entry in &context.entries {
    if entry.record_type == "dataset" {
      datasets.push(entry.record_id.clone());
    } else if !entry.dataset_id.is_empty() {
      datasets.push(entry.dataset_id.clone());
    }
    if entry.record_type == "variable" {
      variables.push(BTreeSet::from([
        entry.record_id.clone(),
        entry.title.clone(),
      ]));
    }
    if !entry.source_url.is_empty() {
      citations.push(entry.source_url.clone());
    }
  }
  (datasets, variables, citations)
}

fn average_metric<F, E>(
  results: &[QuestionEvaluationResult],
  questions: &[BenchmarkQuestion],
  value: F,
  expected: E,
) -> f64
where
  F: Fn(&QuestionEvaluationResult) -> f64,
  E: Fn(&BenchmarkQuestion) -> &[String],
{
  let values = results
    .iter()
    .filter_map(|result| {
      questions
        .iter()
        .find(|question| question.question_id == result.question_id)
        .filter(|question| !expected(question).is_empty())
        .map(|_| value(result))
    })
    .collect::<Vec<_>>();
  if values.is_empty() {
    1.0
  } else {
    ratio_sum(values.iter().sum::<f64>(), values.len())
  }
}

#[allow(clippy::cast_precision_loss)]
fn ratio_sum(sum: f64, denominator: usize) -> f64 {
  sum / denominator as f64
}

fn mean_result(
  results: &[QuestionEvaluationResult],
  questions: &[BenchmarkQuestion],
  path: impl Fn(&QuestionEvaluationResult) -> &PathEvaluationResult,
) -> PathEvaluationResult {
  PathEvaluationResult {
    dataset_recall_at_5: average_metric(
      results,
      questions,
      |result| path(result).dataset_recall_at_5,
      |question| &question.expected_datasets,
    ),
    variable_recall_at_5: average_metric(
      results,
      questions,
      |result| path(result).variable_recall_at_5,
      |question| &question.expected_variables,
    ),
    citation_accuracy: average_metric(
      results,
      questions,
      |result| path(result).citation_accuracy,
      |question| &question.expected_citations,
    ),
    dataset_mrr: average_metric(
      results,
      questions,
      |result| path(result).dataset_mrr,
      |question| &question.expected_datasets,
    ),
    variable_mrr: average_metric(
      results,
      questions,
      |result| path(result).variable_mrr,
      |question| &question.expected_variables,
    ),
  }
}

/// Runs benchmark queries across lexical, hybrid-fallback, and agent-facing paths.
///
/// # Errors
///
/// Returns an error when retrieval fails.
pub fn evaluate_benchmark_suite(
  config: &VariableEvaluationConfig,
  suite: &BenchmarkQuestionSuite,
) -> Result<BenchmarkReport, AppError> {
  config.validate()?;
  let results = suite
    .questions
    .iter()
    .map(|question| {
      let lexical_results = run_retrieval(&config.retrieval, &question.query, 10)?;
      let (lexical_datasets, lexical_variables, lexical_citations) =
        extract_search_ids(&lexical_results);
      let lexical = evaluate_path(
        &lexical_datasets,
        &lexical_variables,
        &lexical_citations,
        question,
      );

      let agent_results = run_retrieval(&config.retrieval, &question.query, 10)?;
      let context = build_agent_context(&question.query, agent_results);
      let (agent_datasets, agent_variables, agent_citations) = extract_agent_ids(&context);
      let agent_facing = evaluate_path(
        &agent_datasets,
        &agent_variables,
        &agent_citations,
        question,
      );

      Ok(QuestionEvaluationResult {
        question_id: question.question_id.clone(),
        query: question.query.clone(),
        lexical: lexical.clone(),
        hybrid: lexical,
        agent_facing,
      })
    })
    .collect::<Result<Vec<_>, AppError>>()?;

  Ok(BenchmarkReport {
    mean_lexical: mean_result(&results, &suite.questions, |result| &result.lexical),
    mean_hybrid: mean_result(&results, &suite.questions, |result| &result.hybrid),
    mean_agent_facing: mean_result(&results, &suite.questions, |result| &result.agent_facing),
    results,
  })
}

/// Reads a benchmark question suite from a JSON file.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed.
pub fn read_benchmark_suite(path: &Path) -> Result<BenchmarkQuestionSuite, AppError> {
  let content = fs::read_to_string(path)
    .map_err(|error| evaluation_error(format!("failed to read {}: {error}", path.display())))?;
  let questions = serde_json::from_str::<Vec<BenchmarkQuestion>>(&content)
    .map_err(|error| evaluation_error(format!("failed to parse {}: {error}", path.display())))?;
  Ok(BenchmarkQuestionSuite { questions })
}

/// Saves a Markdown benchmark report.
///
/// # Errors
///
/// Returns an error if the report cannot be written.
pub fn generate_markdown_report(
  report: &BenchmarkReport,
  output_path: &Path,
) -> Result<(), AppError> {
  if let Some(parent) = output_path.parent() {
    fs::create_dir_all(parent).map_err(|error| {
      evaluation_error(format!("failed to create {}: {error}", parent.display()))
    })?;
  }
  let mut lines = vec![
    "# Retrieval Performance Evaluation Report".to_string(),
    String::new(),
    "This report compares retrieval and citation performance across implemented search paths."
      .to_string(),
    String::new(),
    "## Aggregate Benchmark Summary".to_string(),
    String::new(),
    "| Metric | Lexical | Hybrid | Agent-facing |".to_string(),
    "| :--- | :---: | :---: | :---: |".to_string(),
    format!(
      "| **Dataset Recall@5** | {:.2}% | {:.2}% | {:.2}% |",
      report.mean_lexical.dataset_recall_at_5 * 100.0,
      report.mean_hybrid.dataset_recall_at_5 * 100.0,
      report.mean_agent_facing.dataset_recall_at_5 * 100.0
    ),
    format!(
      "| **Variable Recall@5** | {:.2}% | {:.2}% | {:.2}% |",
      report.mean_lexical.variable_recall_at_5 * 100.0,
      report.mean_hybrid.variable_recall_at_5 * 100.0,
      report.mean_agent_facing.variable_recall_at_5 * 100.0
    ),
    format!(
      "| **Citation Accuracy** | {:.2}% | {:.2}% | {:.2}% |",
      report.mean_lexical.citation_accuracy * 100.0,
      report.mean_hybrid.citation_accuracy * 100.0,
      report.mean_agent_facing.citation_accuracy * 100.0
    ),
    format!(
      "| **Dataset MRR** | {:.4} | {:.4} | {:.4} |",
      report.mean_lexical.dataset_mrr,
      report.mean_hybrid.dataset_mrr,
      report.mean_agent_facing.dataset_mrr
    ),
    format!(
      "| **Variable MRR** | {:.4} | {:.4} | {:.4} |",
      report.mean_lexical.variable_mrr,
      report.mean_hybrid.variable_mrr,
      report.mean_agent_facing.variable_mrr
    ),
    String::new(),
    "## Per-Question Results Comparison".to_string(),
    String::new(),
  ];

  for result in &report.results {
    lines.extend([
      format!("### Query: `{}` (ID: {})", result.query, result.question_id),
      String::new(),
      "| Path | Dataset Recall@5 | Variable Recall@5 | Citation Accuracy | Dataset MRR | Variable MRR |".to_string(),
      "| :--- | :---: | :---: | :---: | :---: | :---: |".to_string(),
      format_path_row("Lexical", &result.lexical),
      format_path_row("Hybrid", &result.hybrid),
      format_path_row("Agent-facing", &result.agent_facing),
      String::new(),
    ]);
  }

  fs::write(output_path, lines.join("\n") + "\n").map_err(|error| {
    evaluation_error(format!(
      "failed to write {}: {error}",
      output_path.display()
    ))
  })
}

fn format_path_row(label: &str, result: &PathEvaluationResult) -> String {
  format!(
    "| **{label}** | {:.2}% | {:.2}% | {:.2}% | {:.4} | {:.4} |",
    result.dataset_recall_at_5 * 100.0,
    result.variable_recall_at_5 * 100.0,
    result.citation_accuracy * 100.0,
    result.dataset_mrr,
    result.variable_mrr
  )
}

/// Formats variable evaluation text output.
#[must_use]
pub fn format_variable_report_text(report: &VariableEvaluationReport) -> String {
  let mut lines = vec![format!(
    "variable retrieval usefulness: {}/{} passed (seed={}, limit={})",
    report.passed_count(),
    report.sample_size,
    report.seed,
    report.limit
  )];
  for case in &report.cases {
    let status = if case.passed { "PASS" } else { "FAIL" };
    let rank = case
      .first_matching_rank
      .map_or_else(|| "-".to_string(), |rank| rank.to_string());
    lines.push(format!("{status}\t{}\trank={rank}", case.variable_name));
  }
  lines.join("\n")
}

/// Formats benchmark text output.
#[must_use]
pub fn format_benchmark_report_text(report: &BenchmarkReport, output_path: &Path) -> String {
  [
    "CMS Retrieval Benchmark Suite Evaluation".to_string(),
    "========================================".to_string(),
    "Lexical Path:".to_string(),
    format_summary_lines(&report.mean_lexical),
    String::new(),
    "Hybrid Path:".to_string(),
    format_summary_lines(&report.mean_hybrid),
    String::new(),
    "Agent-facing Path:".to_string(),
    format_summary_lines(&report.mean_agent_facing),
    String::new(),
    format!("Comparative report saved to {}", output_path.display()),
  ]
  .join("\n")
}

fn format_summary_lines(result: &PathEvaluationResult) -> String {
  [
    format!(
      "  Mean Dataset Recall@5: {:.2}%",
      result.dataset_recall_at_5 * 100.0
    ),
    format!(
      "  Mean Variable Recall@5: {:.2}%",
      result.variable_recall_at_5 * 100.0
    ),
    format!(
      "  Mean Citation Accuracy: {:.2}%",
      result.citation_accuracy * 100.0
    ),
    format!("  Mean Dataset MRR: {:.4}", result.dataset_mrr),
    format!("  Mean Variable MRR: {:.4}", result.variable_mrr),
  ]
  .join("\n")
}
