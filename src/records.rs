//! Typed domain records matching canonical CSV/JSONL structures.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct InventoryRow {
  pub url: String,
  pub title: String,
  pub resource_kind: String,
  pub asset_kind: Option<String>,
  pub content_type: String,
  pub http_status: Option<u16>,
  pub link_state: String,
  pub linked_documents: Option<usize>,
  pub source_url: Option<String>,
  pub source_title: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct InventoryEdgeRow {
  pub source_url: String,
  pub target_url: String,
  pub relationship: String,
  pub source_title: Option<String>,
  pub target_title: Option<String>,
  pub target_resource_kind: String,
  pub target_asset_kind: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ArchiveManifestRow {
  pub url: String,
  pub resource_kind: String,
  pub asset_kind: Option<String>,
  pub source_url: Option<String>,
  pub source_title: Option<String>,
  pub content_type: Option<String>,
  pub http_status: Option<u16>,
  pub archive_state: String,
  pub downloaded_at_utc: Option<String>,
  pub sha256: Option<String>,
  pub local_path: Option<String>,
  pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct DatasetMetadataRow {
  pub dataset_id: String,
  pub name: String,
  pub program: String,
  pub category: String,
  pub availability: String,
  pub source_url: String,
  pub local_path: String,
  pub sha256: String,
  pub extraction_notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct DocumentMetadataRow {
  pub document_id: String,
  pub dataset_id: String,
  pub title: String,
  pub document_kind: String,
  pub source_url: String,
  pub local_path: String,
  pub sha256: String,
  pub content_type: Option<String>,
  pub extraction_notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct DocumentEdgeRow {
  pub source_id: String,
  pub target_id: String,
  pub relationship: String,
  pub source_url: String,
  pub local_path: String,
  pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct VariableMetadataRow {
  pub variable_id: String,
  pub variable_name: String,
  pub dataset_id: String,
  pub definition: String,
  pub aliases: Option<String>,
  pub years: Option<String>,
  pub source_document: String,
  pub source_url: String,
  pub page: Option<usize>,
  pub chunk_id: String,
  pub extraction_notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct VariableEdgeRow {
  pub source_id: String,
  pub target_id: String,
  pub relationship: String,
  pub source_url: String,
  pub source_document: String,
  pub page: Option<usize>,
  pub chunk_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct CanonicalVariableRow {
  pub variable_id: String,
  pub variable_name: String,
  pub variable_label: String,
  pub definition: String,
  pub source: String,
  pub source_url: String,
  pub source_document: String,
  pub extraction_notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct DataSourceVariableEdgeRow {
  pub source_id: String,
  pub target_id: String,
  pub relationship: String,
  pub source_url: String,
  pub source_document: String,
  pub variable_url: String,
  pub variable_document: String,
  pub evidence_type: String,
  pub page: Option<usize>,
  pub chunk_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct OntologyNodeRow {
  pub node_id: String,
  pub node_class: String,
  pub name: String,
  pub source_url: String,
  pub local_path: String,
  pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct OntologyEdgeRow {
  pub source_id: String,
  pub target_id: String,
  pub relationship: String,
  pub source_url: String,
  pub local_path: String,
  pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ChunkMetadata {
  pub chunk_id: String,
  pub source_document: String,
  pub page: Option<usize>,
  pub text: String,
  pub dataset: String,
  pub url: String,
}
