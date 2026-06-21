#![allow(
  clippy::missing_errors_doc,
  clippy::must_use_candidate,
  clippy::implicit_hasher,
  clippy::if_not_else,
  clippy::module_name_repetitions,
  clippy::too_many_lines,
  clippy::cast_possible_truncation,
  clippy::cast_sign_loss,
  clippy::cast_precision_loss,
  clippy::manual_string_new,
  clippy::items_after_statements,
  unused_imports
)]
use rkb::archive::{DownloadResult, run_archive};
use rkb::config::{ArchiveConfig, InventoryConfig};
use rkb::error::AppError;
use rkb::inventory::{
  HtmlFetchResult, ProbeResult, classify_asset_kind, classify_resource_kind, crawl_inventory,
  parse_page,
};
use rkb::records::{ArchiveManifestRow, InventoryEdgeRow, InventoryRow};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn test_resource_classification() {
  assert_eq!(
    classify_resource_kind("https://resdac.org/cms-data"),
    "listing_page"
  );
  assert_eq!(
    classify_resource_kind("https://resdac.org/cms-data/files/some-dataset"),
    "dataset_page"
  );
  assert_eq!(
    classify_resource_kind("https://resdac.org/cms-data/files/some-dataset/data-documentation"),
    "documentation_page"
  );
  assert_eq!(
    classify_resource_kind("https://resdac.org/cms-data/variables/some-var"),
    "variable_page"
  );
  assert_eq!(
    classify_resource_kind("https://resdac.org/cms-data/files/some-dataset/file.pdf"),
    "dataset_page"
  );
  assert_eq!(
    classify_resource_kind("https://www2.ccwdata.org/documents/10280/19022436/codebook-pde.pdf"),
    "asset"
  );
  assert_eq!(
    classify_resource_kind("https://resdac.org/cms-data/some-other-path"),
    "other"
  );

  assert_eq!(
    classify_asset_kind("https://example.com/file.pdf", None),
    "pdf"
  );
  assert_eq!(
    classify_asset_kind("https://example.com/file", Some("application/pdf")),
    "pdf"
  );
  assert_eq!(
    classify_asset_kind("https://example.com/file.xlsx", None),
    "xlsx"
  );
  assert_eq!(
    classify_asset_kind("https://example.com/file.xls", None),
    "xls"
  );
  assert_eq!(
    classify_asset_kind("https://example.com/file.csv", None),
    "csv"
  );
  assert_eq!(
    classify_asset_kind("https://example.com/file.zip", None),
    "zip"
  );
  assert_eq!(
    classify_asset_kind("https://example.com/file.png", None),
    "other"
  );
}

#[test]
fn test_html_link_extraction() {
  let html = r#"
    <!DOCTYPE html>
    <html>
      <head><title>Test Title | ResDAC</title></head>
      <body>
        <h1>Heading Title</h1>
        <a href="/cms-data/files/dataset-1">Dataset 1</a>
        <a href="https://resdac.org/cms-data/variables/var-1">  Variable 1  </a>
        <a href="/some-unrelated-path">Other</a>
      </body>
    </html>
  "#;

  let (title, links) = parse_page(html);
  assert_eq!(title, "Test Title | ResDAC");
  assert_eq!(links.len(), 3);
  assert_eq!(links[0].href, "/cms-data/files/dataset-1");
  assert_eq!(links[0].text, "Dataset 1");
  assert_eq!(links[1].href, "https://resdac.org/cms-data/variables/var-1");
  assert_eq!(links[1].text, "Variable 1");
}

#[test]
fn test_crawl_inventory_mocked() {
  let mut config = InventoryConfig {
    base_url: "https://resdac.org/cms-data".to_string(),
    max_pages: 2,
    max_follow_pages: Some(2),
    max_assets: Some(2),
    workspace_dir: PathBuf::from("_workspace_test_inventory"),
    ..InventoryConfig::default()
  };
  let _ = config.validate();

  // Pre-populate mock HTML pages and responses
  let mut mock_pages = HashMap::new();
  mock_pages.insert(
    "https://resdac.org/cms-data?page=0".to_string(),
    (
      200,
      r#"
      <title>Listing Page 0</title>
      <a href="/cms-data/files/dataset-a">Dataset A</a>
      <a href="/cms-data/files/dataset-b">Dataset B</a>
    "#
      .to_string(),
    ),
  );
  mock_pages.insert(
    "https://resdac.org/cms-data?page=1".to_string(),
    (
      200,
      r#"
      <title>Listing Page 1</title>
      <a href="/cms-data/files/dataset-c">Dataset C</a>
    "#
      .to_string(),
    ),
  );
  mock_pages.insert(
    "https://resdac.org/cms-data/files/dataset-a".to_string(),
    (
      200,
      r#"
      <title>Dataset A Page</title>
      <a href="/cms-data/files/dataset-a/data-documentation">Documentation A</a>
      <a href="/cms-data/files/dataset-a/file.pdf">PDF Asset</a>
    "#
      .to_string(),
    ),
  );
  mock_pages.insert(
    "https://resdac.org/cms-data/files/dataset-b".to_string(),
    (
      200,
      r#"
      <title>Dataset B Page</title>
      <a href="/cms-data/files/dataset-b/file.xlsx">Excel Asset</a>
    "#
      .to_string(),
    ),
  );
  mock_pages.insert(
    "https://resdac.org/cms-data/files/dataset-a/data-documentation".to_string(),
    (
      200,
      r#"
      <title>Documentation A Page</title>
      <a href="/cms-data/variables/var-x">Variable X</a>
    "#
      .to_string(),
    ),
  );

  let fetch_html =
    move |url: &str, _timeout: f64, _user_agent: &str| -> Result<HtmlFetchResult, AppError> {
      if let Some((status, html)) = mock_pages.get(url) {
        Ok(HtmlFetchResult {
          url: url.to_string(),
          status: *status,
          content_type: "text/html".to_string(),
          html: html.clone(),
        })
      } else {
        Ok(HtmlFetchResult {
          url: url.to_string(),
          status: 404,
          content_type: "text/html".to_string(),
          html: "".to_string(),
        })
      }
    };

  let probe_url = |_url: &str, _timeout: f64, _user_agent: &str| -> Result<ProbeResult, AppError> {
    Ok(ProbeResult {
      status: 200,
      content_type: Some("application/pdf".to_string()),
    })
  };

  let result = crawl_inventory(&config, fetch_html, probe_url, None).expect("crawl failed");

  // max_pages is 2, so listing page 0 and 1 are crawled.
  // max_follow_pages is 2, so only 2 dataset/doc pages are followed (e.g. dataset-a, dataset-b).
  // Check counts:
  assert!(!result.rows.is_empty());
  assert!(!result.edges.is_empty());
}

#[test]
fn test_archive_mocked() {
  let test_dir = PathBuf::from("_workspace/test_runs_mocked")
    .join(chrono::Utc::now().timestamp_millis().to_string());
  let workspace_dir = test_dir.join("workspace");
  let raw_root = test_dir.join("raw");
  let inventory_path = test_dir.join("inventory.csv");
  let manifest_output_path = test_dir.join("manifest.csv");

  let inventory = vec![InventoryRow {
    url: "https://resdac.org/cms-data/files/dataset-a".to_string(),
    title: "Dataset A".to_string(),
    resource_kind: "dataset_page".to_string(),
    asset_kind: None,
    content_type: "text/html".to_string(),
    http_status: Some(200),
    link_state: "live".to_string(),
    linked_documents: Some(0),
    source_url: None,
    source_title: None,
  }];

  rkb::inventory::write_inventory_csv(&inventory, &inventory_path).unwrap();

  let config = ArchiveConfig {
    inventory_path,
    raw_root: raw_root.clone(),
    manifest_output_path: manifest_output_path.clone(),
    workspace_dir,
    timeout_seconds: 20.0,
    request_delay_seconds: 0.0,
    max_consecutive_rate_limits: 5,
    retry_failed_only: false,
    max_downloads: None,
    rate_limit_cooldown_seconds: 0.0,
    progress_log_path: None,
    progress_interval: 25,
    user_agent: "TestAgent".to_string(),
  };

  let download_url =
    |_url: &str, _timeout: f64, _user_agent: &str| -> Result<DownloadResult, AppError> {
      Ok(DownloadResult {
        status: 200,
        content_type: Some("text/html".to_string()),
        body: b"<html>Dataset Page</html>".to_vec(),
      })
    };

  let sleep_fn = |_secs: f64| {};

  let (result, summary_path) = run_archive(&config, download_url, sleep_fn, None).unwrap();
  assert_eq!(result.archived_count, 1);
  assert_eq!(result.manifest_rows.len(), 1);
  assert_eq!(result.manifest_rows[0].archive_state, "archived");
  assert!(manifest_output_path.is_file());
  assert!(summary_path.is_file());

  // Verify file was written to correct path: html/dataset_page/<slug>.html
  let slug = rkb::archive::slug_for_row(&inventory[0]);
  let expected_file_path = raw_root
    .join("html")
    .join("dataset_page")
    .join(format!("{slug}.html"));
  assert!(expected_file_path.is_file());
  assert_eq!(
    fs::read_to_string(expected_file_path).unwrap(),
    "<html>Dataset Page</html>"
  );

  // Clean up
  let _ = fs::remove_dir_all(test_dir);
}

#[test]
fn test_archive_rate_limiting_and_retry() {
  use std::sync::atomic::{AtomicUsize, Ordering};

  let test_dir = PathBuf::from("_workspace/test_runs_rate_limit")
    .join(chrono::Utc::now().timestamp_millis().to_string());
  let workspace_dir = test_dir.join("workspace");
  let raw_root = test_dir.join("raw");
  let inventory_path = test_dir.join("inventory.csv");
  let manifest_output_path = test_dir.join("manifest.csv");

  let inventory = vec![
    InventoryRow {
      url: "https://resdac.org/cms-data/variables/var1".to_string(),
      title: "Var 1".to_string(),
      resource_kind: "variable_page".to_string(),
      asset_kind: None,
      content_type: "text/html".to_string(),
      http_status: Some(200),
      link_state: "live".to_string(),
      linked_documents: Some(0),
      source_url: None,
      source_title: None,
    },
    InventoryRow {
      url: "https://resdac.org/cms-data/variables/var2".to_string(),
      title: "Var 2".to_string(),
      resource_kind: "variable_page".to_string(),
      asset_kind: None,
      content_type: "text/html".to_string(),
      http_status: Some(200),
      link_state: "live".to_string(),
      linked_documents: Some(0),
      source_url: None,
      source_title: None,
    },
  ];

  rkb::inventory::write_inventory_csv(&inventory, &inventory_path).unwrap();

  let config = ArchiveConfig {
    inventory_path,
    raw_root,
    manifest_output_path,
    workspace_dir,
    timeout_seconds: 20.0,
    request_delay_seconds: 0.0,
    max_consecutive_rate_limits: 1, // Circuit break after 1 rate limit!
    retry_failed_only: false,
    max_downloads: None,
    rate_limit_cooldown_seconds: 0.05,
    progress_log_path: None,
    progress_interval: 25,
    user_agent: "TestAgent".to_string(),
  };

  use std::sync::Arc;

  let call_count = Arc::new(AtomicUsize::new(0));
  let call_count_clone = Arc::clone(&call_count);
  let download_url =
    move |_url: &str, _timeout: f64, _user_agent: &str| -> Result<DownloadResult, AppError> {
      call_count_clone.fetch_add(1, Ordering::SeqCst);
      Ok(DownloadResult {
        status: 429,
        content_type: Some("text/html".to_string()),
        body: Vec::new(),
      })
    };

  let sleep_calls = Arc::new(AtomicUsize::new(0));
  let sleep_calls_clone = Arc::clone(&sleep_calls);
  let sleep_fn = move |_secs: f64| {
    sleep_calls_clone.fetch_add(1, Ordering::SeqCst);
  };

  let (result, _) = run_archive(&config, download_url, sleep_fn, None).unwrap();

  // The first row will be downloaded, return 429, increment consecutive rate limits to 1.
  // Since max_consecutive_rate_limits is 1, circuit breaker will trip on the first variable_page.
  // The second row will be bulk-deferred immediately without any download attempt!
  assert_eq!(call_count.load(Ordering::SeqCst), 1);
  assert_eq!(result.manifest_rows.len(), 2);
  assert_eq!(result.manifest_rows[0].archive_state, "failed"); // The one that returned 429
  assert_eq!(result.manifest_rows[1].archive_state, "deferred"); // The circuit-broken one

  // Sleep calls should be 1 for rate-limit cooldown
  assert_eq!(sleep_calls.load(Ordering::SeqCst), 1);

  // Clean up
  let _ = fs::remove_dir_all(test_dir);
}
