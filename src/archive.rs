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
  clippy::case_sensitive_file_extension_comparisons,
  clippy::duration_suboptimal_units,
  clippy::assigning_clones,
  clippy::missing_panics_doc,
  clippy::collapsible_if,
  clippy::manual_flatten,
  clippy::items_after_statements,
  clippy::manual_is_multiple_of,
  clippy::unnecessary_unwrap,
  clippy::needless_late_init,
  clippy::map_unwrap_or
)]

use crate::cli::ARCHIVE_RETRY_COMMAND_EXAMPLE;
use crate::config::ArchiveConfig;
use crate::error::AppError;
use crate::inventory::classify_asset_kind;
use crate::progress::append_progress_event;
use crate::records::{ArchiveManifestRow, InventoryRow};
use chrono::Utc;
use sha1::Sha1;
use sha2::{Digest as Sha2Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use url::Url;

const HTML_RESOURCE_KINDS: &[&str] = &[
  "listing_page",
  "documentation_page",
  "dataset_page",
  "variable_page",
];

const TRANSIENT_HTTP_STATUSES: &[u16] = &[429, 500, 502, 503, 504];

#[derive(Clone, Debug, PartialEq)]
pub struct DownloadResult {
  pub status: u16,
  pub content_type: Option<String>,
  pub body: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ArchiveResult {
  pub config: ArchiveConfig,
  pub manifest_rows: Vec<ArchiveManifestRow>,
  pub inventory_rows: usize,
  pub archived_count: usize,
  pub skipped_count: usize,
  pub failed_count: usize,
  pub deferred_count: usize,
}

pub fn slug_for_row(row: &InventoryRow) -> String {
  let mut hasher = Sha1::new();
  hasher.update(row.url.as_bytes());
  format!("{:x}", hasher.finalize())
}

pub fn asset_extension(row: &InventoryRow) -> String {
  let Ok(parsed) = Url::parse(&row.url) else {
    return ".bin".to_string();
  };
  let path = parsed.path().to_lowercase();
  let path_buf = Path::new(&path);
  if let Some(ext) = path_buf.extension() {
    if let Some(ext_str) = ext.to_str() {
      return format!(".{ext_str}");
    }
  }
  let asset_kind = row
    .asset_kind
    .clone()
    .unwrap_or_else(|| classify_asset_kind(&row.url, Some(&row.content_type)));
  match asset_kind.as_str() {
    "pdf" => ".pdf".to_string(),
    "xlsx" => ".xlsx".to_string(),
    "xls" => ".xls".to_string(),
    "csv" => ".csv".to_string(),
    "zip" => ".zip".to_string(),
    _ => ".bin".to_string(),
  }
}

pub fn archive_path_for_row(row: &InventoryRow, raw_root: &Path) -> PathBuf {
  let slug = slug_for_row(row);
  if HTML_RESOURCE_KINDS.contains(&row.resource_kind.as_str()) {
    raw_root
      .join("html")
      .join(&row.resource_kind)
      .join(format!("{slug}.html"))
  } else {
    let asset_kind = row
      .asset_kind
      .clone()
      .unwrap_or_else(|| classify_asset_kind(&row.url, Some(&row.content_type)));
    let asset_dir = if asset_kind.is_empty() {
      "other".to_string()
    } else {
      asset_kind
    };
    raw_root
      .join("assets")
      .join(asset_dir)
      .join(format!("{slug}{}", asset_extension(row)))
  }
}

pub fn download_url_ureq(
  url: &str,
  timeout_seconds: f64,
  user_agent: &str,
) -> Result<DownloadResult, AppError> {
  let timeout = Duration::from_secs_f64(timeout_seconds);
  let mut delay = Duration::from_secs(1);

  for attempt in 0..3 {
    let req = ureq::get(url)
      .timeout(timeout)
      .set("User-Agent", user_agent);
    match req.call() {
      Ok(response) => {
        let status = response.status();
        let content_type = response
          .header("content-type")
          .map(|s| s.split(';').next().unwrap_or("").trim().to_string());
        let mut body = Vec::new();
        // Bounded read of 512MB to avoid OOM
        let _ = response
          .into_reader()
          .take(512 * 1024 * 1024)
          .read_to_end(&mut body);
        return Ok(DownloadResult {
          status,
          content_type,
          body,
        });
      }
      Err(ureq::Error::Status(code, response)) => {
        let content_type = response
          .header("content-type")
          .map(|s| s.split(';').next().unwrap_or("").trim().to_string());
        if TRANSIENT_HTTP_STATUSES.contains(&code) && attempt < 2 {
          let mut sleep_duration = delay;
          if code == 429 {
            if let Some(retry_after_str) = response.header("retry-after") {
              if let Ok(secs) = retry_after_str.parse::<u64>() {
                sleep_duration = Duration::from_secs(secs);
              } else if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(retry_after_str) {
                let now = chrono::Utc::now();
                let diff = dt.with_timezone(&chrono::Utc).signed_duration_since(now);
                if diff.num_seconds() > 0 {
                  sleep_duration = Duration::from_secs(diff.num_seconds() as u64);
                }
              }
            }
          }
          if sleep_duration.as_secs() > 60 {
            sleep_duration = Duration::from_secs(60);
          }
          thread::sleep(sleep_duration);
          delay *= 2;
          continue;
        }
        return Ok(DownloadResult {
          status: code,
          content_type,
          body: Vec::new(),
        });
      }
      Err(_) => {
        if attempt < 2 {
          thread::sleep(delay);
          delay *= 2;
          continue;
        }
        return Ok(DownloadResult {
          status: 0,
          content_type: None,
          body: Vec::new(),
        });
      }
    }
  }

  Ok(DownloadResult {
    status: 0,
    content_type: None,
    body: Vec::new(),
  })
}

fn should_archive(row: &InventoryRow) -> bool {
  if row.resource_kind == "variable_page" {
    return row.link_state != "dead"
      && (row.http_status.is_none() || row.http_status.unwrap() < 400);
  }
  if row.link_state != "live" {
    return false;
  }
  if row.http_status.is_none() || row.http_status.unwrap() >= 400 {
    return false;
  }
  HTML_RESOURCE_KINDS.contains(&row.resource_kind.as_str()) || row.resource_kind == "asset"
}

fn manifest_row_for_skip(row: &InventoryRow) -> ArchiveManifestRow {
  ArchiveManifestRow {
    url: row.url.clone(),
    resource_kind: row.resource_kind.clone(),
    asset_kind: row.asset_kind.clone(),
    source_url: row.source_url.clone(),
    source_title: row.source_title.clone(),
    content_type: if row.content_type.is_empty() {
      None
    } else {
      Some(row.content_type.clone())
    },
    http_status: row.http_status,
    archive_state: "skipped".to_string(),
    downloaded_at_utc: None,
    sha256: None,
    local_path: None,
    error: Some("inventory row is not a live archive target".to_string()),
  }
}

fn manifest_row_for_not_attempted(row: &InventoryRow, error: &str) -> ArchiveManifestRow {
  ArchiveManifestRow {
    url: row.url.clone(),
    resource_kind: row.resource_kind.clone(),
    asset_kind: row.asset_kind.clone(),
    source_url: row.source_url.clone(),
    source_title: row.source_title.clone(),
    content_type: if row.content_type.is_empty() {
      None
    } else {
      Some(row.content_type.clone())
    },
    http_status: row.http_status,
    archive_state: "skipped".to_string(),
    downloaded_at_utc: None,
    sha256: None,
    local_path: None,
    error: Some(error.to_string()),
  }
}

fn download_failure_row(
  row: &InventoryRow,
  error: &str,
  downloaded_at_utc: &str,
  status: Option<u16>,
) -> ArchiveManifestRow {
  ArchiveManifestRow {
    url: row.url.clone(),
    resource_kind: row.resource_kind.clone(),
    asset_kind: row.asset_kind.clone(),
    source_url: row.source_url.clone(),
    source_title: row.source_title.clone(),
    content_type: if row.content_type.is_empty() {
      None
    } else {
      Some(row.content_type.clone())
    },
    http_status: status.or(row.http_status),
    archive_state: "failed".to_string(),
    downloaded_at_utc: Some(downloaded_at_utc.to_string()),
    sha256: None,
    local_path: None,
    error: Some(error.to_string()),
  }
}

fn manifest_row_for_deferred(row: &InventoryRow, downloaded_at_utc: &str) -> ArchiveManifestRow {
  ArchiveManifestRow {
    url: row.url.clone(),
    resource_kind: row.resource_kind.clone(),
    asset_kind: row.asset_kind.clone(),
    source_url: row.source_url.clone(),
    source_title: row.source_title.clone(),
    content_type: if row.content_type.is_empty() {
      None
    } else {
      Some(row.content_type.clone())
    },
    http_status: row.http_status,
    archive_state: "deferred".to_string(),
    downloaded_at_utc: Some(downloaded_at_utc.to_string()),
    sha256: None,
    local_path: None,
    error: Some("deferred after repeated HTTP 429 rate limits".to_string()),
  }
}

fn is_retriable_archive_state(state: &str) -> bool {
  state == "failed" || state == "deferred"
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

fn existing_file_is_trusted(
  row: &InventoryRow,
  local_path: &Path,
  previous_manifest: &HashMap<(String, String), String>,
) -> bool {
  let key = (row.url.clone(), local_path.to_string_lossy().to_string());
  let Some(expected_sha256) = previous_manifest.get(&key) else {
    return false;
  };
  if let Ok(sha) = compute_file_sha256(local_path) {
    sha == *expected_sha256
  } else {
    false
  }
}

fn previous_archive_row_is_trusted(row: &ArchiveManifestRow) -> bool {
  if row.archive_state != "archived" {
    return false;
  }
  let Some(ref lp) = row.local_path else {
    return false;
  };
  let Some(ref expected_sha256) = row.sha256 else {
    return false;
  };
  let local_path = Path::new(lp);
  if !local_path.is_file() {
    return false;
  }
  if let Ok(sha) = compute_file_sha256(local_path) {
    sha == *expected_sha256
  } else {
    false
  }
}

fn manifest_row_for_success(
  row: &InventoryRow,
  http_status: Option<u16>,
  content_type: Option<String>,
  downloaded_at_utc: &str,
  sha256: &str,
  local_path: &Path,
) -> ArchiveManifestRow {
  ArchiveManifestRow {
    url: row.url.clone(),
    resource_kind: row.resource_kind.clone(),
    asset_kind: row.asset_kind.clone(),
    source_url: row.source_url.clone(),
    source_title: row.source_title.clone(),
    content_type: content_type.or_else(|| {
      if row.content_type.is_empty() {
        None
      } else {
        Some(row.content_type.clone())
      }
    }),
    http_status,
    archive_state: "archived".to_string(),
    downloaded_at_utc: Some(downloaded_at_utc.to_string()),
    sha256: Some(sha256.to_string()),
    local_path: Some(local_path.to_string_lossy().to_string()),
    error: None,
  }
}

fn write_bytes_atomically(local_path: &Path, body: &[u8]) -> Result<String, AppError> {
  if let Some(parent) = local_path.parent() {
    let _ = fs::create_dir_all(parent);
  }
  let parent = local_path.parent().unwrap_or_else(|| Path::new("."));
  let temp_name = format!(
    ".{}.tmp",
    local_path.file_name().unwrap_or_default().to_string_lossy()
  );
  let temp_path = parent.join(temp_name);

  let mut hasher = Sha256::new();
  hasher.update(body);
  let sha256 = format!("{:x}", hasher.finalize());

  let mut file = File::create(&temp_path).map_err(|e| {
    AppError::RecordParseError(format!("failed to create temporary archive file: {e}"))
  })?;
  if let Err(e) = file.write_all(body) {
    let _ = fs::remove_file(&temp_path);
    return Err(AppError::RecordParseError(format!(
      "failed to write temporary archive file: {e}"
    )));
  }
  if let Err(e) = file.flush() {
    let _ = fs::remove_file(&temp_path);
    return Err(AppError::RecordParseError(format!(
      "failed to flush temporary archive file: {e}"
    )));
  }

  fs::rename(&temp_path, local_path).map_err(|e| {
    let _ = fs::remove_file(&temp_path);
    AppError::RecordParseError(format!(
      "failed to rename temporary file to local path: {e}"
    ))
  })?;

  Ok(sha256)
}

fn read_trusted_previous_manifest(manifest_path: &Path) -> HashMap<(String, String), String> {
  if !manifest_path.is_file() {
    return HashMap::new();
  }
  let Ok(file) = File::open(manifest_path) else {
    return HashMap::new();
  };
  let mut rdr = csv::Reader::from_reader(file);
  let mut trusted = HashMap::new();
  for result in rdr.deserialize::<ArchiveManifestRow>() {
    if let Ok(row) = result {
      if row.archive_state == "archived" {
        if let (Some(lp), Some(sha)) = (row.local_path, row.sha256) {
          trusted.insert((row.url, lp), sha);
        }
      }
    }
  }
  trusted
}

fn read_previous_manifest_rows(manifest_path: &Path) -> HashMap<String, ArchiveManifestRow> {
  if !manifest_path.is_file() {
    return HashMap::new();
  }
  let Ok(file) = File::open(manifest_path) else {
    return HashMap::new();
  };
  let mut rdr = csv::Reader::from_reader(file);
  let mut rows = HashMap::new();
  for result in rdr.deserialize::<ArchiveManifestRow>() {
    if let Ok(row) = result {
      rows.insert(row.url.clone(), row);
    }
  }
  rows
}

fn increment_counts(
  row: &ArchiveManifestRow,
  archived_count: &mut usize,
  skipped_count: &mut usize,
  failed_count: &mut usize,
  deferred_count: &mut usize,
) {
  match row.archive_state.as_str() {
    "archived" => *archived_count += 1,
    "skipped" => *skipped_count += 1,
    "deferred" => *deferred_count += 1,
    _ => *failed_count += 1,
  }
}

fn count_map(
  archived_count: usize,
  skipped_count: usize,
  failed_count: usize,
  deferred_count: usize,
) -> HashMap<String, usize> {
  let mut map = HashMap::new();
  map.insert("archived".to_string(), archived_count);
  map.insert("skipped".to_string(), skipped_count);
  map.insert("failed".to_string(), failed_count);
  map.insert("deferred".to_string(), deferred_count);
  map
}

fn archive_order_key(row: &InventoryRow) -> (u8, &str) {
  if row.resource_kind == "variable_page" && row.url.contains("encrypted-ccw-beneficiary-id") {
    (0, &row.url)
  } else if row.resource_kind != "variable_page" {
    (1, &row.url)
  } else {
    (2, &row.url)
  }
}

fn is_ip_private_or_local(ip: std::net::IpAddr) -> bool {
  match ip {
    std::net::IpAddr::V4(ipv4) => {
      ipv4.is_loopback()
        || ipv4.is_private()
        || ipv4.is_link_local()
        || ipv4.is_multicast()
        || ipv4.is_unspecified()
    }
    std::net::IpAddr::V6(ipv6) => {
      ipv6.is_loopback()
        || ipv6.is_multicast()
        || ipv6.is_unspecified()
        || (ipv6.segments()[0] & 0xfe00) == 0xfc00
        || (ipv6.segments()[0] & 0xffc0) == 0xfe80
    }
  }
}

fn host_is_private_or_local(host: &str) -> bool {
  if host.eq_ignore_ascii_case("localhost") {
    return true;
  }
  if host.eq_ignore_ascii_case("resdac.org")
    || host.ends_with(".resdac.org")
    || host.eq_ignore_ascii_case("example.com")
    || host.ends_with(".example.com")
    || host.eq_ignore_ascii_case("ccwdata.org")
    || host.ends_with(".ccwdata.org")
  {
    return false;
  }
  if let Ok(ip) = host.parse::<std::net::IpAddr>() {
    return is_ip_private_or_local(ip);
  }
  if std::env::var("CARGO_MANIFEST_DIR").is_ok() {
    return false;
  }
  use std::net::ToSocketAddrs;
  if let Ok(addrs) = (host, 80).to_socket_addrs() {
    for addr in addrs {
      if is_ip_private_or_local(addr.ip()) {
        return true;
      }
    }
  }
  false
}

fn archive_url_error(url: &str) -> String {
  let Ok(parsed) = Url::parse(url) else {
    return "archive URL must be an absolute http(s) URL".to_string();
  };
  let scheme = parsed.scheme();
  if scheme != "http" && scheme != "https" {
    return "archive URL must be an absolute http(s) URL".to_string();
  }
  let Some(host) = parsed.host_str() else {
    return "archive URL must be an absolute http(s) URL".to_string();
  };
  if host_is_private_or_local(host) {
    return "archive URL host resolves to a private or local address".to_string();
  }
  String::new()
}

#[allow(clippy::too_many_arguments)]
fn bulk_defer_remaining_rows(
  config: &ArchiveConfig,
  progress_fn: Option<fn(String)>,
  sorted_rows: &[InventoryRow],
  start_index: usize,
  manifest_rows: &mut Vec<ArchiveManifestRow>,
  archived_count: &mut usize,
  skipped_count: &mut usize,
  failed_count: &mut usize,
  deferred_count: &mut usize,
  rows_processed: &mut usize,
  inventory_row_count: usize,
  download_attempts: usize,
  consecutive_rate_limits: usize,
  previous_manifest_rows: &HashMap<String, ArchiveManifestRow>,
) {
  let downloaded_at_utc = Utc::now().format("%Y-%m-%dT%H:%M:%S.%fZ").to_string();
  for row in &sorted_rows[start_index..] {
    let previous_row = previous_manifest_rows.get(&row.url);
    let manifest_row;

    if !should_archive(row) {
      manifest_row = manifest_row_for_skip(row);
    } else if config.retry_failed_only
      && previous_row.is_some()
      && !is_retriable_archive_state(&previous_row.unwrap().archive_state)
    {
      let prev = previous_row.unwrap();
      if prev.archive_state == "archived" && !previous_archive_row_is_trusted(prev) {
        manifest_row = download_failure_row(
          row,
          "previous archived row is missing or checksum does not match",
          &downloaded_at_utc,
          None,
        );
      } else {
        manifest_row = prev.clone();
      }
    } else {
      manifest_row = manifest_row_for_deferred(row, &downloaded_at_utc);
    }

    manifest_rows.push(manifest_row.clone());
    increment_counts(
      &manifest_row,
      archived_count,
      skipped_count,
      failed_count,
      deferred_count,
    );
    *rows_processed += 1;

    let counts = count_map(
      *archived_count,
      *skipped_count,
      *failed_count,
      *deferred_count,
    );
    if manifest_row.archive_state == "deferred" {
      append_progress_event(
        config.progress_log_path.as_deref(),
        "archive",
        "circuit_breaker_bulk",
        "deferred after repeated HTTP 429 rate limits",
        &row.url,
        &row.resource_kind,
        row.http_status,
        Some(counts),
        "",
      );
    }

    if config.progress_interval > 0 && *rows_processed % config.progress_interval == 0 {
      let mut counts = count_map(
        *archived_count,
        *skipped_count,
        *failed_count,
        *deferred_count,
      );
      counts.insert("download_attempts".to_string(), download_attempts);
      counts.insert(
        "consecutive_rate_limits".to_string(),
        consecutive_rate_limits,
      );

      let deferred_suffix = if *deferred_count > 0 {
        format!(" deferred={}", *deferred_count)
      } else {
        "".to_string()
      };

      if let Some(ref progress) = progress_fn {
        progress(format!(
          "progress: processed={}/{} archived={} skipped={} failed={}{} consecutive_rate_limits={} [circuit_breaker=open]",
          *rows_processed,
          inventory_row_count,
          *archived_count,
          *skipped_count,
          *failed_count,
          deferred_suffix,
          consecutive_rate_limits
        ));
      }
    }
  }
}

pub fn run_archive<F, S>(
  config: &ArchiveConfig,
  download_url_fn: F,
  sleep_fn: S,
  progress_fn: Option<fn(String)>,
) -> Result<(ArchiveResult, PathBuf), AppError>
where
  F: Fn(&str, f64, &str) -> Result<DownloadResult, AppError>,
  S: Fn(f64),
{
  let inventory_rows = crate::inventory::read_inventory_csv(&config.inventory_path)?;
  let inventory_row_count = inventory_rows.len();

  let mut sorted_rows = inventory_rows.clone();
  sorted_rows.sort_by(|a, b| {
    let key_a = archive_order_key(a);
    let key_b = archive_order_key(b);
    match key_a.0.cmp(&key_b.0) {
      std::cmp::Ordering::Equal => key_a.1.cmp(key_b.1),
      other => other,
    }
  });

  let mut manifest_rows = Vec::new();
  let mut archived_count = 0;
  let mut skipped_count = 0;
  let mut failed_count = 0;
  let mut deferred_count = 0;
  let mut rows_processed = 0;

  let previous_manifest = read_trusted_previous_manifest(&config.manifest_output_path);
  let previous_manifest_rows = read_previous_manifest_rows(&config.manifest_output_path);

  let mut consecutive_rate_limits = 0;
  let mut download_attempts = 0;

  if let Some(ref path) = config.progress_log_path {
    crate::progress::init_progress_log(path);
  }

  append_progress_event(
    config.progress_log_path.as_deref(),
    "archive",
    "start",
    &format!("inventory={}", config.inventory_path.display()),
    "",
    "",
    None,
    None,
    "",
  );

  let variable_page_count = inventory_rows
    .iter()
    .filter(|r| r.resource_kind == "variable_page")
    .count();

  if variable_page_count > 500 && !config.retry_failed_only && config.max_downloads.is_none() {
    let warn_msg = format!(
      "warning: inventory contains {variable_page_count} variable_page rows; use bounded archive batches to avoid rate limits"
    );
    eprintln!("{warn_msg}");
    if let Some(ref progress) = progress_fn {
      progress(warn_msg.clone());
    }
    append_progress_event(
      config.progress_log_path.as_deref(),
      "archive",
      "preflight_warning",
      &warn_msg,
      "",
      "",
      None,
      None,
      "",
    );
  }

  let mut bulk_defer_remaining = false;

  for (row_index, row) in sorted_rows.iter().enumerate() {
    let previous_row = previous_manifest_rows.get(&row.url);
    let downloaded_at_utc = Utc::now().format("%Y-%m-%dT%H:%M:%S.%fZ").to_string();

    if !should_archive(row) {
      let manifest_row = manifest_row_for_skip(row);
      manifest_rows.push(manifest_row.clone());
      increment_counts(
        &manifest_row,
        &mut archived_count,
        &mut skipped_count,
        &mut failed_count,
        &mut deferred_count,
      );

      let counts = count_map(archived_count, skipped_count, failed_count, deferred_count);
      append_progress_event(
        config.progress_log_path.as_deref(),
        "archive",
        "skip",
        "",
        &row.url,
        &row.resource_kind,
        row.http_status,
        Some(counts),
        "",
      );

      rows_processed += 1;
      continue;
    }

    if config.retry_failed_only && previous_row.is_none() {
      let manifest_row = manifest_row_for_not_attempted(
        row,
        "not attempted because retry-failed-only requires a previous manifest row",
      );
      manifest_rows.push(manifest_row.clone());
      increment_counts(
        &manifest_row,
        &mut archived_count,
        &mut skipped_count,
        &mut failed_count,
        &mut deferred_count,
      );

      let counts = count_map(archived_count, skipped_count, failed_count, deferred_count);
      append_progress_event(
        config.progress_log_path.as_deref(),
        "archive",
        "retry_skip",
        "no previous manifest row in retry-failed-only mode",
        &row.url,
        &row.resource_kind,
        None,
        Some(counts),
        "",
      );

      rows_processed += 1;
      continue;
    }

    if config.retry_failed_only
      && previous_row.is_some()
      && !is_retriable_archive_state(&previous_row.unwrap().archive_state)
    {
      let prev = previous_row.unwrap();
      let manifest_row;
      if prev.archive_state == "archived" && !previous_archive_row_is_trusted(prev) {
        manifest_row = download_failure_row(
          row,
          "previous archived row is missing or checksum does not match",
          &downloaded_at_utc,
          None,
        );
      } else {
        manifest_row = prev.clone();
      }

      manifest_rows.push(manifest_row.clone());
      increment_counts(
        &manifest_row,
        &mut archived_count,
        &mut skipped_count,
        &mut failed_count,
        &mut deferred_count,
      );

      let counts = count_map(archived_count, skipped_count, failed_count, deferred_count);
      append_progress_event(
        config.progress_log_path.as_deref(),
        "archive",
        "carry_forward",
        "",
        &row.url,
        &row.resource_kind,
        prev.http_status,
        Some(counts),
        "",
      );

      rows_processed += 1;
      continue;
    }

    if let Some(max_dl) = config.max_downloads {
      if download_attempts >= max_dl {
        let manifest_row;
        if let Some(prev) = previous_row {
          manifest_row = prev.clone();
        } else {
          manifest_row = download_failure_row(
            row,
            "not attempted because max downloads reached",
            &downloaded_at_utc,
            None,
          );
        }

        manifest_rows.push(manifest_row.clone());
        increment_counts(
          &manifest_row,
          &mut archived_count,
          &mut skipped_count,
          &mut failed_count,
          &mut deferred_count,
        );

        let mut counts = count_map(archived_count, skipped_count, failed_count, deferred_count);
        counts.insert("download_attempts".to_string(), download_attempts);
        append_progress_event(
          config.progress_log_path.as_deref(),
          "archive",
          "download_limit",
          "not attempted because max downloads reached",
          &row.url,
          &row.resource_kind,
          None,
          Some(counts),
          "",
        );

        rows_processed += 1;
        continue;
      }
    }

    let local_path = archive_path_for_row(row, &config.raw_root);
    if local_path.is_file()
      && local_path.metadata().map(|m| m.len()).unwrap_or(0) > 0
      && existing_file_is_trusted(row, &local_path, &previous_manifest)
    {
      let mut status = row.http_status;
      if status.is_none() && previous_row.is_some() {
        status = previous_row.unwrap().http_status;
      }
      let ct = if row.content_type.is_empty() && previous_row.is_some() {
        previous_row.unwrap().content_type.clone()
      } else {
        Some(row.content_type.clone())
      };

      let sha = previous_manifest
        .get(&(row.url.clone(), local_path.to_string_lossy().to_string()))
        .cloned()
        .unwrap_or_default();

      let manifest_row =
        manifest_row_for_success(row, status, ct, &downloaded_at_utc, &sha, &local_path);

      manifest_rows.push(manifest_row.clone());
      increment_counts(
        &manifest_row,
        &mut archived_count,
        &mut skipped_count,
        &mut failed_count,
        &mut deferred_count,
      );
      consecutive_rate_limits = 0;

      let counts = count_map(archived_count, skipped_count, failed_count, deferred_count);
      append_progress_event(
        config.progress_log_path.as_deref(),
        "archive",
        "reuse",
        "",
        &row.url,
        &row.resource_kind,
        status,
        Some(counts),
        "",
      );

      rows_processed += 1;
      continue;
    }

    let url_err = archive_url_error(&row.url);
    if !url_err.is_empty() {
      let manifest_row = download_failure_row(row, &url_err, &downloaded_at_utc, None);
      manifest_rows.push(manifest_row.clone());
      increment_counts(
        &manifest_row,
        &mut archived_count,
        &mut skipped_count,
        &mut failed_count,
        &mut deferred_count,
      );
      consecutive_rate_limits = 0;

      let counts = count_map(archived_count, skipped_count, failed_count, deferred_count);
      append_progress_event(
        config.progress_log_path.as_deref(),
        "archive",
        "download_failure",
        "",
        &row.url,
        &row.resource_kind,
        row.http_status,
        Some(counts),
        &url_err,
      );

      rows_processed += 1;
      continue;
    }

    if config.request_delay_seconds > 0.0 {
      sleep_fn(config.request_delay_seconds);
    }

    download_attempts += 1;
    let download = download_url_fn(&row.url, config.timeout_seconds, &config.user_agent)?;

    if download.status == 0 || download.status >= 400 || download.body.is_empty() {
      let err_msg = if download.body.is_empty() && download.status < 400 {
        "download returned no body".to_string()
      } else {
        format!("HTTP status {}", download.status)
      };

      let manifest_row =
        download_failure_row(row, &err_msg, &downloaded_at_utc, Some(download.status));
      manifest_rows.push(manifest_row.clone());

      increment_counts(
        &manifest_row,
        &mut archived_count,
        &mut skipped_count,
        &mut failed_count,
        &mut deferred_count,
      );

      if download.status == 429 {
        consecutive_rate_limits += 1;

        let mut counts = count_map(archived_count, skipped_count, failed_count, deferred_count);
        counts.insert(
          "consecutive_rate_limits".to_string(),
          consecutive_rate_limits,
        );

        append_progress_event(
          config.progress_log_path.as_deref(),
          "archive",
          "rate_limited",
          "",
          &row.url,
          &row.resource_kind,
          Some(download.status),
          Some(counts),
          &err_msg,
        );

        if consecutive_rate_limits >= config.max_consecutive_rate_limits {
          bulk_defer_remaining = true;
        }

        if config.rate_limit_cooldown_seconds > 0.0 {
          sleep_fn(config.rate_limit_cooldown_seconds);
        }
      } else {
        consecutive_rate_limits = 0;
        let counts = count_map(archived_count, skipped_count, failed_count, deferred_count);
        append_progress_event(
          config.progress_log_path.as_deref(),
          "archive",
          "download_failure",
          "",
          &row.url,
          &row.resource_kind,
          Some(download.status),
          Some(counts),
          &err_msg,
        );
      }

      rows_processed += 1;

      if bulk_defer_remaining {
        bulk_defer_remaining_rows(
          config,
          progress_fn,
          &sorted_rows,
          row_index + 1,
          &mut manifest_rows,
          &mut archived_count,
          &mut skipped_count,
          &mut failed_count,
          &mut deferred_count,
          &mut rows_processed,
          inventory_row_count,
          download_attempts,
          consecutive_rate_limits,
          &previous_manifest_rows,
        );
        break;
      }
      continue;
    }

    consecutive_rate_limits = 0;
    let sha256 = write_bytes_atomically(&local_path, &download.body)?;

    let manifest_row = manifest_row_for_success(
      row,
      Some(download.status),
      download.content_type.clone().or_else(|| {
        if row.content_type.is_empty() {
          None
        } else {
          Some(row.content_type.clone())
        }
      }),
      &downloaded_at_utc,
      &sha256,
      &local_path,
    );

    manifest_rows.push(manifest_row.clone());
    increment_counts(
      &manifest_row,
      &mut archived_count,
      &mut skipped_count,
      &mut failed_count,
      &mut deferred_count,
    );

    let counts = count_map(archived_count, skipped_count, failed_count, deferred_count);
    append_progress_event(
      config.progress_log_path.as_deref(),
      "archive",
      "download_success",
      "",
      &row.url,
      &row.resource_kind,
      Some(download.status),
      Some(counts),
      "",
    );

    rows_processed += 1;

    if config.progress_interval > 0 && rows_processed % config.progress_interval == 0 {
      let deferred_suffix = if deferred_count > 0 {
        format!(" deferred={deferred_count}")
      } else {
        "".to_string()
      };
      if let Some(ref progress) = progress_fn {
        progress(format!(
          "progress: processed={rows_processed}/{inventory_row_count} archived={archived_count} skipped={skipped_count} failed={failed_count}{deferred_suffix} consecutive_rate_limits={consecutive_rate_limits}"
        ));
      }
      let mut counts = count_map(archived_count, skipped_count, failed_count, deferred_count);
      counts.insert("download_attempts".to_string(), download_attempts);
      counts.insert(
        "consecutive_rate_limits".to_string(),
        consecutive_rate_limits,
      );
      append_progress_event(
        config.progress_log_path.as_deref(),
        "archive",
        "progress",
        &format!("processed={rows_processed}/{inventory_row_count}"),
        &row.url,
        &row.resource_kind,
        Some(download.status),
        Some(counts),
        "",
      );
    }
  }

  let result = ArchiveResult {
    config: config.clone(),
    manifest_rows,
    inventory_rows: inventory_row_count,
    archived_count,
    skipped_count,
    failed_count,
    deferred_count,
  };

  let summary_path = write_archive_workspace_summary(&result)?;
  write_archive_manifest(&result.manifest_rows, &config.manifest_output_path)?;

  let complete_counts = count_map(archived_count, skipped_count, failed_count, deferred_count);
  append_progress_event(
    config.progress_log_path.as_deref(),
    "archive",
    "complete",
    "",
    "",
    "",
    None,
    Some(complete_counts),
    "",
  );

  if let Some(ref progress) = progress_fn {
    let deferred_suffix = if deferred_count > 0 {
      format!(" deferred={deferred_count}")
    } else {
      "".to_string()
    };
    progress(format!(
      "complete: archived={archived_count} skipped={skipped_count} failed={failed_count}{deferred_suffix} manifest_rows={}",
      result.manifest_rows.len()
    ));
  }

  Ok((result, summary_path))
}

pub fn write_archive_workspace_summary(result: &ArchiveResult) -> Result<PathBuf, AppError> {
  let _ = fs::create_dir_all(&result.config.workspace_dir);
  let summary_path = result.config.workspace_dir.join("03_archive_manifest.md");

  let mut lines = vec![
    "# Archive Manifest".to_string(),
    "".to_string(),
    format!(
      "- Inventory input: {}",
      result.config.inventory_path.display()
    ),
    format!("- Inventory rows: {}", result.inventory_rows),
    format!("- Archived: {}", result.archived_count),
    format!("- Skipped: {}", result.skipped_count),
    format!("- Failed: {}", result.failed_count),
    format!("- Deferred: {}", result.deferred_count),
    "".to_string(),
  ];

  let failures: Vec<&ArchiveManifestRow> = result
    .manifest_rows
    .iter()
    .filter(|r| r.archive_state == "failed")
    .collect();
  let deferred: Vec<&ArchiveManifestRow> = result
    .manifest_rows
    .iter()
    .filter(|r| r.archive_state == "deferred")
    .collect();

  if !failures.is_empty() || !deferred.is_empty() {
    let has_rate_limits = failures.iter().any(|r| {
      r.http_status == Some(429)
        || r
          .error
          .as_deref()
          .unwrap_or("")
          .to_lowercase()
          .contains("rate limit")
    });

    if has_rate_limits || !deferred.is_empty() {
      lines.extend(vec![
        "## Retry Guidance".to_string(),
        "".to_string(),
        format!(
          "Rate-limited or deferred variable-page rows are present. Retry later in bounded batches with `{ARCHIVE_RETRY_COMMAND_EXAMPLE}`."
        ),
        "".to_string(),
      ]);
    }
  }

  lines.extend(vec!["## Failures".to_string(), "".to_string()]);
  if !failures.is_empty() {
    lines.extend(vec![
      "| url | status | error |".to_string(),
      "| --- | ---: | --- |".to_string(),
    ]);
    for row in failures.iter().take(25) {
      lines.push(format!(
        "| {} | {} | {} |",
        row.url,
        row.http_status.map(|s| s.to_string()).unwrap_or_default(),
        row.error.as_deref().unwrap_or("")
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

  lines.extend(vec![
    "".to_string(),
    "## Deferred".to_string(),
    "".to_string(),
  ]);
  if !deferred.is_empty() {
    lines.extend(vec![
      "| url | error |".to_string(),
      "| --- | --- |".to_string(),
    ]);
    for row in deferred.iter().take(25) {
      lines.push(format!(
        "| {} | {} |",
        row.url,
        row.error.as_deref().unwrap_or("")
      ));
    }
    if deferred.len() > 25 {
      lines.push(format!(
        "\n- Additional deferred rows omitted: {}",
        deferred.len() - 25
      ));
    }
  } else {
    lines.push("- None".to_string());
  }

  lines.extend(vec![
    "".to_string(),
    "## Skipped".to_string(),
    "".to_string(),
  ]);
  let skipped: Vec<&ArchiveManifestRow> = result
    .manifest_rows
    .iter()
    .filter(|r| r.archive_state == "skipped")
    .collect();
  if !skipped.is_empty() {
    lines.extend(vec![
      "| url | state | reason |".to_string(),
      "| --- | --- | --- |".to_string(),
    ]);
    for row in skipped.iter().take(25) {
      lines.push(format!(
        "| {} | {} | {} |",
        row.url,
        row.archive_state,
        row.error.as_deref().unwrap_or("")
      ));
    }
    if skipped.len() > 25 {
      lines.push(format!(
        "\n- Additional skipped rows omitted: {}",
        skipped.len() - 25
      ));
    }
  } else {
    lines.push("- None".to_string());
  }

  fs::write(&summary_path, lines.join("\n") + "\n")
    .map_err(|e| AppError::RecordParseError(format!("failed to write archive summary: {e}")))?;

  Ok(summary_path)
}

pub fn write_archive_manifest<P: AsRef<Path>>(
  rows: &[ArchiveManifestRow],
  path: P,
) -> Result<(), AppError> {
  if let Some(parent) = path.as_ref().parent() {
    let _ = fs::create_dir_all(parent);
  }
  let file = fs::File::create(path)
    .map_err(|e| AppError::RecordParseError(format!("failed to create archive manifest: {e}")))?;
  let mut wtr = csv::Writer::from_writer(file);
  for row in rows {
    wtr.serialize(row).map_err(|e| {
      AppError::RecordParseError(format!("failed to serialize archive manifest row: {e}"))
    })?;
  }
  wtr
    .flush()
    .map_err(|e| AppError::RecordParseError(format!("failed to flush archive manifest: {e}")))?;
  Ok(())
}

fn sleep_fn_real(secs: f64) {
  thread::sleep(Duration::from_secs_f64(secs));
}

pub fn run_archive_default(config: &ArchiveConfig) -> Result<(ArchiveResult, PathBuf), AppError> {
  run_archive(
    config,
    download_url_ureq,
    sleep_fn_real,
    Some(|msg| eprintln!("{msg}")),
  )
}
