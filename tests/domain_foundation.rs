use rkb::AppError;
use rkb::config::{ArchiveConfig, InventoryConfig, ParsingConfig};
use rkb::paths::get_packaged_data_path;
use rkb::records::{ArchiveManifestRow, ChunkMetadata, InventoryRow};
use std::fs;
use std::path::{Path, PathBuf};

fn baseline_path() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/python-baseline")
}

#[test]
fn test_config_base_url_validation() {
  // Test valid URL with trailing slash stripped
  let mut config = InventoryConfig {
    base_url: "https://resdac.org/cms-data/".to_string(),
    ..InventoryConfig::default()
  };
  assert!(config.validate().is_ok());
  assert_eq!(config.base_url, "https://resdac.org/cms-data");

  // Test invalid URL (no scheme)
  let mut config2 = InventoryConfig {
    base_url: "resdac.org/cms-data".to_string(),
    ..InventoryConfig::default()
  };
  assert!(matches!(
    config2.validate(),
    Err(AppError::ConfigValidationError(_))
  ));

  // Test invalid URL (relative path)
  let mut config3 = InventoryConfig {
    base_url: "/cms-data".to_string(),
    ..InventoryConfig::default()
  };
  assert!(matches!(
    config3.validate(),
    Err(AppError::ConfigValidationError(_))
  ));
}

#[test]
fn test_config_limits_validation() {
  // Inventory limits
  let mut inv_config = InventoryConfig {
    max_pages: 0,
    ..InventoryConfig::default()
  };
  assert!(matches!(
    inv_config.validate(),
    Err(AppError::ConfigValidationError(_))
  ));

  // Archive limits
  let arch_config = ArchiveConfig {
    timeout_seconds: 0.0,
    ..ArchiveConfig::default()
  };
  assert!(matches!(
    arch_config.validate(),
    Err(AppError::ConfigValidationError(_))
  ));

  let arch_config2 = ArchiveConfig {
    max_consecutive_rate_limits: 0,
    ..ArchiveConfig::default()
  };
  assert!(matches!(
    arch_config2.validate(),
    Err(AppError::ConfigValidationError(_))
  ));

  // Parsing limits
  let parse_config = ParsingConfig {
    chunk_size: 0,
    ..ParsingConfig::default()
  };
  assert!(matches!(
    parse_config.validate(),
    Err(AppError::ConfigValidationError(_))
  ));

  let parse_config2 = ParsingConfig {
    chunk_size: 500,
    chunk_overlap: 500,
    ..ParsingConfig::default()
  };
  assert!(matches!(
    parse_config2.validate(),
    Err(AppError::ConfigValidationError(_))
  ));
}

#[test]
fn test_path_resolution() {
  let subpath = "metadata/datasets.csv";
  let resolved = get_packaged_data_path(subpath);
  assert_eq!(resolved, Path::new("data").join(subpath));
}

#[test]
fn test_site_inventory_csv_roundtrip() {
  let path = baseline_path().join("site_inventory.csv");
  let content = fs::read_to_string(&path).expect("failed to read site_inventory.csv");

  let mut rdr = csv::Reader::from_reader(content.as_bytes());
  let mut records = Vec::new();
  for result in rdr.deserialize::<InventoryRow>() {
    let record = result.expect("failed to deserialize InventoryRow");
    records.push(record);
  }

  assert!(
    !records.is_empty(),
    "expected non-empty site inventory rows"
  );
  assert_eq!(records[0].url, "https://resdac.org/cms-data?page=0");
  assert_eq!(records[0].resource_kind, "listing_page");
  assert_eq!(records[0].http_status, Some(200));

  // Serialize back to CSV
  let mut wtr = csv::Writer::from_writer(Vec::new());
  for record in &records {
    wtr
      .serialize(record)
      .expect("failed to serialize InventoryRow");
  }
  let serialized = wtr.into_inner().expect("failed to finalize serialization");
  assert!(!serialized.is_empty());
}

#[test]
fn test_archive_manifest_csv_roundtrip() {
  let path = baseline_path().join("archive_manifest.csv");
  let content = fs::read_to_string(&path).expect("failed to read archive_manifest.csv");

  let mut rdr = csv::Reader::from_reader(content.as_bytes());
  let mut records = Vec::new();
  for result in rdr.deserialize::<ArchiveManifestRow>() {
    let record = result.expect("failed to deserialize ArchiveManifestRow");
    records.push(record);
  }

  assert!(
    !records.is_empty(),
    "expected non-empty archive manifest rows"
  );
  assert_eq!(records[0].resource_kind, "variable_page");
  assert_eq!(records[0].archive_state, "archived");
  assert_eq!(
    records[0].sha256,
    Some("afd7f96c4b33e954afbec9e56af4cd890b0723504ea7ec4bf31d9f4615e01dad".to_string())
  );
}

#[test]
fn test_chunk_metadata_jsonl_roundtrip() {
  let path = baseline_path().join("chunks.jsonl");
  let content = fs::read_to_string(&path).expect("failed to read chunks.jsonl");

  let mut records = Vec::new();
  for line in content.lines() {
    if line.trim().is_empty() {
      continue;
    }
    let record: ChunkMetadata = serde_json::from_str(line).expect("failed to parse JSONL line");
    records.push(record);
  }

  assert!(
    !records.is_empty(),
    "expected non-empty chunk metadata rows"
  );
  assert_eq!(records[0].chunk_id, "ahc-model__chunk_0");
  assert_eq!(records[0].dataset, "ahc-model");
}
