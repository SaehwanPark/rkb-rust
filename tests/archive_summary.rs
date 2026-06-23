use rkb::archive::{ArchiveResult, write_archive_workspace_summary};
use rkb::cli::ARCHIVE_RETRY_COMMAND_EXAMPLE;
use rkb::config::ArchiveConfig;
use rkb::records::ArchiveManifestRow;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn archive_retry_guidance_uses_rkb_subcommand() {
  let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
  let workspace_dir = std::env::temp_dir().join(format!(
    "rkb-archive-summary-{}-{sequence}",
    std::process::id()
  ));
  fs::create_dir_all(&workspace_dir).expect("workspace should be created");

  let result = ArchiveResult {
    config: ArchiveConfig {
      workspace_dir: workspace_dir.clone(),
      ..ArchiveConfig::default()
    },
    manifest_rows: vec![ArchiveManifestRow {
      url: "https://resdac.org/cms-data/variables/example".to_string(),
      resource_kind: "variable_page".to_string(),
      asset_kind: None,
      source_url: None,
      source_title: None,
      content_type: None,
      http_status: Some(429),
      archive_state: "deferred".to_string(),
      downloaded_at_utc: None,
      sha256: None,
      local_path: None,
      error: Some("deferred after repeated HTTP 429 rate limits".to_string()),
    }],
    inventory_rows: 1,
    archived_count: 0,
    skipped_count: 0,
    failed_count: 0,
    deferred_count: 1,
  };

  let summary_path = write_archive_workspace_summary(&result).expect("summary should write");
  let summary = fs::read_to_string(&summary_path).expect("summary should be readable");

  assert!(summary.contains("## Retry Guidance"));
  assert!(summary.contains(ARCHIVE_RETRY_COMMAND_EXAMPLE));
  assert!(summary.contains("rkb archive --retry-failed-only"));
  assert!(!summary.contains("cms-kb-archive"));
  assert!(!summary.contains("uv run"));

  let _ = fs::remove_dir_all(workspace_dir);
}
