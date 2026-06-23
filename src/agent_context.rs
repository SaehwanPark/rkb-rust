//! Agent-oriented formatting for citation-bearing retrieval results.

use crate::retrieval::SearchResult;
use serde::{Deserialize, Serialize};

/// A complete agent context response for one query.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentContext {
  pub query: String,
  pub result_count: usize,
  pub entries: Vec<AgentContextEntry>,
}

/// One citation-bearing context entry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentContextEntry {
  pub citation: String,
  pub record_id: String,
  pub record_type: String,
  pub title: String,
  pub dataset_id: String,
  pub score: f64,
  pub snippet: String,
  pub source_url: String,
  pub source_document: String,
  pub page: Option<usize>,
}

/// Builds deterministic agent context from retrieval hits.
#[must_use]
pub fn build_agent_context(query: &str, results: Vec<SearchResult>) -> AgentContext {
  let entries = results
    .into_iter()
    .enumerate()
    .map(|(index, result)| AgentContextEntry {
      citation: format!("[{}]", index + 1),
      record_id: result.record_id,
      record_type: result.record_type.as_str().to_string(),
      title: result.title,
      dataset_id: result.dataset_id,
      score: result.score,
      snippet: result.snippet,
      source_url: result.source_url,
      source_document: result.source_document,
      page: result.page,
    })
    .collect::<Vec<_>>();

  AgentContext {
    query: query.to_string(),
    result_count: entries.len(),
    entries,
  }
}

/// Formats agent context as stable plain text.
#[must_use]
pub fn format_agent_context_text(context: &AgentContext) -> String {
  let mut lines = vec![format!("Query: {}", context.query)];
  if context.entries.is_empty() {
    lines.push("No matching context found.".to_string());
    return lines.join("\n");
  }

  for entry in &context.entries {
    let page = entry
      .page
      .map_or_else(String::new, |page| format!(" page {page}"));
    lines.push(format!(
      "{} {} ({}) dataset={} score={:.3}",
      entry.citation, entry.title, entry.record_type, entry.dataset_id, entry.score
    ));
    lines.push(format!("Source: {}{}", entry.source_url, page));
    if !entry.source_document.is_empty() {
      lines.push(format!("Document: {}", entry.source_document));
    }
    lines.push(format!("Snippet: {}", entry.snippet));
  }

  lines.join("\n")
}
