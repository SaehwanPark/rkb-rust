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
  clippy::used_underscore_items,
  clippy::missing_panics_doc,
  clippy::collapsible_if,
  clippy::needless_borrow,
  clippy::case_sensitive_file_extension_comparisons,
  clippy::duration_suboptimal_units,
  clippy::assigning_clones,
  clippy::manual_strip
)]

use crate::config::InventoryConfig;
use crate::error::AppError;
use crate::progress::append_progress_event;
use crate::records::{InventoryEdgeRow, InventoryRow};
use scraper::{Html, Selector};
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use url::Url;

const TRANSIENT_HTTP_STATUSES: &[u16] = &[429, 500, 502, 503, 504];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedLink {
  pub href: String,
  pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HtmlFetchResult {
  pub url: String,
  pub status: u16,
  pub content_type: String,
  pub html: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProbeResult {
  pub status: u16,
  pub content_type: Option<String>,
}

#[derive(Clone, Debug)]
pub struct InventoryResult {
  pub rows: Vec<InventoryRow>,
  pub edges: Vec<InventoryEdgeRow>,
  pub summary: HashMap<String, usize>,
  pub dead_links: Vec<InventoryRow>,
  pub duplicates_skipped: usize,
}

pub fn normalize_whitespace(text: &str) -> String {
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_www(netloc: &str) -> &str {
  if netloc.starts_with("www.") {
    &netloc[4..]
  } else {
    netloc
  }
}

pub fn normalize_url(base_url: &str, href: &str) -> String {
  let Ok(base) = Url::parse(base_url) else {
    return href.to_string();
  };
  let Ok(absolute) = base.join(href) else {
    return href.to_string();
  };

  let scheme = absolute.scheme().to_lowercase();
  let mut host = absolute.host_str().unwrap_or("").to_lowercase();
  host = strip_www(&host).to_string();

  let port = absolute.port();
  let host_with_port = if let Some(p) = port {
    format!("{host}:{p}")
  } else {
    host
  };

  let mut path = absolute.path().to_string();
  if path != "/" && path.ends_with('/') {
    path = path.trim_end_matches('/').to_string();
  }

  let query = absolute.query();

  let mut result = format!("{scheme}://{host_with_port}{path}");
  if let Some(q) = query {
    result.push('?');
    result.push_str(q);
  }
  result
}

pub fn classify_resource_kind(url: &str) -> String {
  let Ok(parsed) = Url::parse(url) else {
    return "other".to_string();
  };
  let path = parsed.path().to_lowercase();
  if path == "/cms-data" {
    return "listing_page".to_string();
  }
  if path.ends_with("/data-documentation") {
    return "documentation_page".to_string();
  }
  if path.starts_with("/cms-data/variables/") {
    return "variable_page".to_string();
  }
  if path.contains("/cms-data/files/") {
    return "dataset_page".to_string();
  }
  if path.ends_with(".pdf")
    || path.ends_with(".xlsx")
    || path.ends_with(".xls")
    || path.ends_with(".csv")
    || path.ends_with(".zip")
  {
    return "asset".to_string();
  }
  "other".to_string()
}

pub fn classify_asset_kind(url: &str, content_type: Option<&str>) -> String {
  let Ok(parsed) = Url::parse(url) else {
    return "other".to_string();
  };
  let path = parsed.path().to_lowercase();
  let ct = content_type.unwrap_or("").to_lowercase();

  if path.ends_with(".pdf") || ct == "application/pdf" {
    return "pdf".to_string();
  }
  if path.ends_with(".xlsx")
    || ct == "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    || ct == "application/vnd.ms-excel"
  {
    return "xlsx".to_string();
  }
  if path.ends_with(".xls") {
    return "xls".to_string();
  }
  if path.ends_with(".csv") || ct == "text/csv" {
    return "csv".to_string();
  }
  if path.ends_with(".zip") || ct == "application/zip" {
    return "zip".to_string();
  }
  "other".to_string()
}

pub fn build_listing_url(base_url: &str, page_number: usize) -> String {
  let Ok(mut url) = Url::parse(base_url) else {
    return base_url.to_string();
  };
  let mut query_pairs: Vec<(String, String)> = url
    .query_pairs()
    .map(|(k, v)| (k.into_owned(), v.into_owned()))
    .collect();

  if let Some(pos) = query_pairs.iter().position(|(k, _)| k == "page") {
    query_pairs[pos].1 = page_number.to_string();
  } else {
    query_pairs.push(("page".to_string(), page_number.to_string()));
  }

  url.set_query(None);
  {
    let mut serializer = url.query_pairs_mut();
    for (k, v) in query_pairs {
      serializer.append_pair(&k, &v);
    }
    serializer.finish();
  }

  url.to_string()
}

pub fn is_relevant_href(base_url: &str, href: &str) -> bool {
  let absolute = normalize_url(base_url, href);
  let Ok(parsed) = Url::parse(&absolute) else {
    return false;
  };
  let Ok(base_parsed) = Url::parse(base_url) else {
    return false;
  };

  let scheme = parsed.scheme();
  if scheme != "http" && scheme != "https" {
    return false;
  }
  let Some(host) = parsed.host_str() else {
    return false;
  };
  let base_host = base_parsed.host_str().unwrap_or("");

  let path = parsed.path().to_lowercase();

  let host1 = strip_www(host).to_lowercase();
  let host2 = strip_www(base_host).to_lowercase();
  let is_same_host = host1 == host2;

  if path.starts_with("/cms-data/files/") || path.starts_with("/cms-data/variables/") {
    return is_same_host;
  }

  if path.ends_with(".pdf")
    || path.ends_with(".xlsx")
    || path.ends_with(".xls")
    || path.ends_with(".csv")
    || path.ends_with(".zip")
  {
    return true;
  }

  false
}

pub fn parse_page(html: &str) -> (String, Vec<ParsedLink>) {
  let document = Html::parse_document(html);

  let title_selector = Selector::parse("title").unwrap();
  let mut title = String::new();
  if let Some(title_element) = document.select(&title_selector).next() {
    title = title_element.text().collect::<Vec<_>>().join("");
  }

  title = normalize_whitespace(&title);
  if title.is_empty() {
    let h1_selector = Selector::parse("h1").unwrap();
    if let Some(h1_element) = document.select(&h1_selector).next() {
      title = h1_element.text().collect::<Vec<_>>().join("");
      title = normalize_whitespace(&title);
    }
  }

  let a_selector = Selector::parse("a").unwrap();
  let mut links = Vec::new();
  for element in document.select(&a_selector) {
    if let Some(href) = element.value().attr("href") {
      let text = element.text().collect::<Vec<_>>().join("");
      links.push(ParsedLink {
        href: href.to_string(),
        text: normalize_whitespace(&text),
      });
    }
  }

  (title, links)
}

pub fn request_with_retry(
  method: &str,
  url: &str,
  headers: &[(String, String)],
  timeout_seconds: f64,
  retry_statuses: &[u16],
  read_body: bool,
) -> (u16, Option<String>, String) {
  let mut delay = Duration::from_secs(1);
  let timeout = Duration::from_secs_f64(timeout_seconds);

  for attempt in 0..3 {
    let mut req = ureq::request(method, url).timeout(timeout);
    for (k, v) in headers {
      req = req.set(k, v);
    }

    match req.call() {
      Ok(response) => {
        let status = response.status();
        let content_type = response
          .header("content-type")
          .map(|s| s.split(';').next().unwrap_or("").trim().to_string());
        let body = if read_body {
          let mut body_str = String::new();
          // Bounded read of 10MB to avoid OOM
          let _ = response
            .into_reader()
            .take(10 * 1024 * 1024)
            .read_to_string(&mut body_str);
          body_str
        } else {
          String::new()
        };
        return (status, content_type, body);
      }
      Err(ureq::Error::Status(code, response)) => {
        let content_type = response
          .header("content-type")
          .map(|s| s.split(';').next().unwrap_or("").trim().to_string());
        if retry_statuses.contains(&code) && attempt < 2 {
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
        return (code, content_type, String::new());
      }
      Err(_) => {
        if attempt < 2 {
          thread::sleep(delay);
          delay *= 2;
          continue;
        }
        return (0, None, String::new());
      }
    }
  }

  (0, None, String::new())
}

pub fn fetch_html_ureq(
  url: &str,
  timeout_seconds: f64,
  user_agent: &str,
) -> Result<HtmlFetchResult, AppError> {
  let headers = vec![("User-Agent".to_string(), user_agent.to_string())];
  let (status, content_type, html) = request_with_retry(
    "GET",
    url,
    &headers,
    timeout_seconds,
    TRANSIENT_HTTP_STATUSES,
    true,
  );
  Ok(HtmlFetchResult {
    url: url.to_string(),
    status,
    content_type: content_type.unwrap_or_else(|| "text/html".to_string()),
    html,
  })
}

pub fn probe_url_ureq(
  url: &str,
  timeout_seconds: f64,
  user_agent: &str,
) -> Result<ProbeResult, AppError> {
  let attempts = vec![
    ("HEAD", vec![]),
    ("GET", vec![("Range".to_string(), "bytes=0-0".to_string())]),
  ];

  for (method, extra_headers) in attempts {
    let mut headers = vec![("User-Agent".to_string(), user_agent.to_string())];
    headers.extend(extra_headers);

    let (status, content_type, _) = request_with_retry(
      method,
      url,
      &headers,
      timeout_seconds,
      TRANSIENT_HTTP_STATUSES,
      false,
    );

    if status == 0 && content_type.is_none() {
      if method == "HEAD" {
        continue;
      }
      return Ok(ProbeResult {
        status: 0,
        content_type: None,
      });
    }

    if method == "HEAD" && (status == 405 || status == 501) {
      continue;
    }

    return Ok(ProbeResult {
      status,
      content_type,
    });
  }

  Ok(ProbeResult {
    status: 0,
    content_type: None,
  })
}

fn link_is_documentation(base_url: &str, href: &str) -> bool {
  classify_resource_kind(&normalize_url(base_url, href)) == "documentation_page"
}

fn link_is_dataset_page(base_url: &str, href: &str) -> bool {
  classify_resource_kind(&normalize_url(base_url, href)) == "dataset_page"
}

fn link_is_asset(base_url: &str, href: &str) -> bool {
  classify_resource_kind(&normalize_url(base_url, href)) == "asset"
}

fn link_is_variable_page(base_url: &str, href: &str) -> bool {
  classify_resource_kind(&normalize_url(base_url, href)) == "variable_page"
}

fn asset_probe_needed(row: &InventoryRow) -> bool {
  row.http_status.is_none() && row.link_state == "unknown"
}

fn link_state_for_status(status: u16) -> String {
  if status < 400 {
    "live".to_string()
  } else if TRANSIENT_HTTP_STATUSES.contains(&status) {
    "unknown".to_string()
  } else {
    "dead".to_string()
  }
}

fn update_probe_row(row: &mut InventoryRow, probe: &ProbeResult) {
  row.http_status = Some(probe.status);
  if let Some(ref ct) = probe.content_type {
    row.content_type = ct.clone();
  }
  row.link_state = link_state_for_status(probe.status);
  row.asset_kind = Some(classify_asset_kind(&row.url, Some(&row.content_type)));
}

fn update_page_row(
  row: &mut InventoryRow,
  title: &str,
  content_type: Option<&str>,
  status: Option<u16>,
  linked_documents: usize,
) {
  if !title.is_empty() && row.title.is_empty() {
    row.title = title.to_string();
  }
  if let Some(ct) = content_type {
    if !ct.is_empty() && row.content_type.is_empty() {
      row.content_type = ct.to_string();
    }
  }
  if let Some(st) = status {
    row.http_status = Some(st);
    row.link_state = link_state_for_status(st);
  }
  row.linked_documents = Some(linked_documents);
}

fn limit_reached(limit: Option<usize>, count: usize) -> bool {
  if let Some(lim) = limit {
    count >= lim
  } else {
    false
  }
}

fn compute_signature(urls: &[String]) -> String {
  let mut sorted_urls = urls.to_vec();
  sorted_urls.sort();
  sorted_urls.dedup();
  let content = sorted_urls.join("\n");
  let mut hasher = Sha1::new();
  hasher.update(content.as_bytes());
  format!("{:x}", hasher.finalize())
}

fn _register_row(
  rows: &mut HashMap<String, InventoryRow>,
  duplicates_skipped: &mut usize,
  url: &str,
  title: &str,
  source_url: &str,
  source_title: &str,
) -> String {
  if let Some(existing) = rows.get_mut(url) {
    *duplicates_skipped += 1;
    if existing.title.is_empty() && !title.is_empty() {
      existing.title = title.to_string();
    }
    if existing.source_url.is_none() && !source_url.is_empty() {
      existing.source_url = Some(source_url.to_string());
    }
    if existing.source_title.is_none() && !source_title.is_empty() {
      existing.source_title = Some(source_title.to_string());
    }
    return url.to_string();
  }

  let kind = classify_resource_kind(url);
  let asset_kind = if kind == "asset" {
    Some(classify_asset_kind(url, None))
  } else {
    None
  };

  let row = InventoryRow {
    url: url.to_string(),
    title: title.to_string(),
    resource_kind: kind,
    asset_kind,
    content_type: String::new(),
    http_status: None,
    link_state: "unknown".to_string(),
    linked_documents: Some(0),
    source_url: if source_url.is_empty() {
      None
    } else {
      Some(source_url.to_string())
    },
    source_title: if source_title.is_empty() {
      None
    } else {
      Some(source_title.to_string())
    },
  };

  rows.insert(url.to_string(), row);
  url.to_string()
}

fn _register_edge(
  edges: &mut HashMap<(String, String, String), InventoryEdgeRow>,
  source_url: &str,
  target_url: &str,
  source_title: &str,
  target_title: &str,
  relationship: &str,
) {
  let target_resource_kind = classify_resource_kind(target_url);
  let target_asset_kind = if target_resource_kind == "asset" {
    Some(classify_asset_kind(target_url, None))
  } else {
    None
  };

  let edge = InventoryEdgeRow {
    source_url: source_url.to_string(),
    target_url: target_url.to_string(),
    relationship: relationship.to_string(),
    source_title: if source_title.is_empty() {
      None
    } else {
      Some(source_title.to_string())
    },
    target_title: if target_title.is_empty() {
      None
    } else {
      Some(target_title.to_string())
    },
    target_resource_kind,
    target_asset_kind,
  };

  edges.insert(
    (
      edge.source_url.clone(),
      edge.target_url.clone(),
      edge.relationship.clone(),
    ),
    edge,
  );
}

fn sorted_rows(mut rows: Vec<InventoryRow>) -> Vec<InventoryRow> {
  let order = |kind: &str| -> usize {
    match kind {
      "listing_page" => 0,
      "dataset_page" => 1,
      "documentation_page" => 2,
      "variable_page" => 3,
      "asset" => 4,
      _ => 5,
    }
  };
  rows.sort_by(|a, b| {
    let order_a = order(&a.resource_kind);
    let order_b = order(&b.resource_kind);
    match order_a.cmp(&order_b) {
      std::cmp::Ordering::Equal => a.url.cmp(&b.url),
      other => other,
    }
  });
  rows
}

fn sorted_edges(mut edges: Vec<InventoryEdgeRow>) -> Vec<InventoryEdgeRow> {
  edges.sort_by(|a, b| match a.source_url.cmp(&b.source_url) {
    std::cmp::Ordering::Equal => match a.target_resource_kind.cmp(&b.target_resource_kind) {
      std::cmp::Ordering::Equal => a.target_url.cmp(&b.target_url),
      other => other,
    },
    other => other,
  });
  edges
}

pub fn crawl_inventory<F, P>(
  config: &InventoryConfig,
  fetch_html_fn: F,
  probe_url_fn: P,
  progress_fn: Option<fn(String)>,
) -> Result<InventoryResult, AppError>
where
  F: Fn(&str, f64, &str) -> Result<HtmlFetchResult, AppError>,
  P: Fn(&str, f64, &str) -> Result<ProbeResult, AppError>,
{
  let mut rows: HashMap<String, InventoryRow> = HashMap::new();
  let mut edges: HashMap<(String, String, String), InventoryEdgeRow> = HashMap::new();
  let mut duplicates_skipped = 0;
  let mut visited_pages: HashSet<String> = HashSet::new();
  let mut seen_listing_signatures: HashSet<String> = HashSet::new();
  let mut queue: VecDeque<String> = VecDeque::new();
  let mut asset_urls: HashSet<String> = HashSet::new();

  let mut listing_pages_fetched = 0;
  let mut follow_pages_fetched = 0;
  let mut asset_probes = 0;

  let max_follow_str = match config.max_follow_pages {
    Some(lim) => lim.to_string(),
    None => "unbounded".to_string(),
  };
  let max_assets_str = match config.max_assets {
    Some(lim) => lim.to_string(),
    None => "unbounded".to_string(),
  };

  if let Some(ref progress) = progress_fn {
    progress(format!(
      "starting inventory crawl: max listing pages={}, max follow pages={}, max assets={}",
      config.max_pages, max_follow_str, max_assets_str
    ));
  }

  append_progress_event(
    config.progress_log_path.as_deref(),
    "inventory",
    "start",
    &format!(
      "max listing pages={}, max follow pages={}, max assets={}",
      config.max_pages, max_follow_str, max_assets_str
    ),
    "",
    "",
    None,
    None,
    "",
  );

  for page_number in 0..config.max_pages {
    let listing_url = build_listing_url(&config.base_url, page_number);
    if config.request_delay_seconds > 0.0 {
      thread::sleep(Duration::from_secs_f64(config.request_delay_seconds));
    }
    let page_result = fetch_html_fn(&listing_url, config.timeout_seconds, &config.user_agent)?;
    listing_pages_fetched += 1;
    let (page_title, links) = parse_page(&page_result.html);

    _register_row(
      &mut rows,
      &mut duplicates_skipped,
      &listing_url,
      &page_title,
      "",
      "",
    );

    {
      let page_row = rows.get_mut(&listing_url).unwrap();
      page_row.resource_kind = "listing_page".to_string();
      update_page_row(
        page_row,
        &page_title,
        Some(&page_result.content_type),
        Some(page_result.status),
        0,
      );
    }

    if page_result.status != 200 {
      break;
    }

    let mut listing_discovered_urls = Vec::new();
    for link in links {
      if !is_relevant_href(&listing_url, &link.href) {
        continue;
      }
      let absolute = normalize_url(&listing_url, &link.href);
      let is_dataset = link_is_dataset_page(&listing_url, &link.href);
      let is_doc = link_is_documentation(&listing_url, &link.href);
      let is_asset = link_is_asset(&listing_url, &link.href);

      if is_dataset || is_doc || is_asset {
        listing_discovered_urls.push(absolute.clone());
        _register_edge(
          &mut edges,
          &listing_url,
          &absolute,
          &page_title,
          &link.text,
          "links_to",
        );
        _register_row(
          &mut rows,
          &mut duplicates_skipped,
          &absolute,
          &link.text,
          &listing_url,
          &page_title,
        );
        if is_dataset {
          if let Some(child_row) = rows.get_mut(&absolute) {
            child_row.resource_kind = "dataset_page".to_string();
          }
          if !visited_pages.contains(&absolute) && !queue.contains(&absolute) {
            queue.push_back(absolute);
          }
        } else if is_doc {
          if let Some(child_row) = rows.get_mut(&absolute) {
            child_row.resource_kind = "documentation_page".to_string();
          }
          if !visited_pages.contains(&absolute) && !queue.contains(&absolute) {
            queue.push_back(absolute);
          }
        } else if is_asset {
          if let Some(child_row) = rows.get_mut(&absolute) {
            child_row.resource_kind = "asset".to_string();
            child_row.asset_kind = Some(classify_asset_kind(&absolute, None));
          }
          asset_urls.insert(absolute);
        }
      }
    }

    if config.progress_interval > 0 && listing_pages_fetched % config.progress_interval == 0 {
      let mut counts = HashMap::new();
      counts.insert("listing_pages".to_string(), listing_pages_fetched);
      counts.insert("follow_pages".to_string(), follow_pages_fetched);
      counts.insert("asset_probes".to_string(), asset_probes);
      counts.insert("rows".to_string(), rows.len());
      counts.insert("edges".to_string(), edges.len());

      append_progress_event(
        config.progress_log_path.as_deref(),
        "inventory",
        "progress",
        &format!("listing pages fetched: {listing_pages_fetched}"),
        &listing_url,
        "listing_page",
        Some(page_result.status),
        Some(counts),
        "",
      );
    }

    let sig = compute_signature(&listing_discovered_urls);
    if page_number > 0 && seen_listing_signatures.contains(&sig) {
      break;
    }
    seen_listing_signatures.insert(sig);
    if listing_discovered_urls.is_empty() && page_number > 0 {
      break;
    }
  }

  while let Some(current_url) = queue.pop_front() {
    if limit_reached(config.max_follow_pages, follow_pages_fetched) {
      if let Some(ref progress) = progress_fn {
        progress(format!(
          "stopped follow-page crawl at configured limit: {}",
          config.max_follow_pages.unwrap()
        ));
      }
      break;
    }
    if visited_pages.contains(&current_url) {
      continue;
    }
    visited_pages.insert(current_url.clone());

    if config.request_delay_seconds > 0.0 {
      thread::sleep(Duration::from_secs_f64(config.request_delay_seconds));
    }
    let page_result = fetch_html_fn(&current_url, config.timeout_seconds, &config.user_agent)?;
    follow_pages_fetched += 1;
    let (page_title, links) = parse_page(&page_result.html);

    _register_row(
      &mut rows,
      &mut duplicates_skipped,
      &current_url,
      &page_title,
      "",
      "",
    );

    let row_kind;
    {
      let row = rows.get_mut(&current_url).unwrap();
      if row.resource_kind == "other" {
        row.resource_kind = classify_resource_kind(&current_url);
      }
      row_kind = row.resource_kind.clone();
      update_page_row(
        row,
        &page_title,
        Some(&page_result.content_type),
        Some(page_result.status),
        0,
      );
    }

    if page_result.status != 200 {
      continue;
    }
    if row_kind == "variable_page" {
      continue;
    }

    let mut page_discovered_urls = Vec::new();
    for link in links {
      if !is_relevant_href(&current_url, &link.href) {
        continue;
      }
      let absolute = normalize_url(&current_url, &link.href);
      if link_is_documentation(&current_url, &link.href) {
        page_discovered_urls.push(absolute.clone());
        _register_edge(
          &mut edges,
          &current_url,
          &absolute,
          &page_title,
          &link.text,
          "links_to",
        );
        _register_row(
          &mut rows,
          &mut duplicates_skipped,
          &absolute,
          &link.text,
          &current_url,
          &page_title,
        );
        {
          let child_row = rows.get_mut(&absolute).unwrap();
          child_row.resource_kind = "documentation_page".to_string();
        }
        if !visited_pages.contains(&absolute) && !queue.contains(&absolute) {
          queue.push_back(absolute);
        }
      } else if link_is_dataset_page(&current_url, &link.href) {
        page_discovered_urls.push(absolute.clone());
        _register_edge(
          &mut edges,
          &current_url,
          &absolute,
          &page_title,
          &link.text,
          "links_to",
        );
        _register_row(
          &mut rows,
          &mut duplicates_skipped,
          &absolute,
          &link.text,
          &current_url,
          &page_title,
        );
        {
          let child_row = rows.get_mut(&absolute).unwrap();
          child_row.resource_kind = "dataset_page".to_string();
        }
        if !visited_pages.contains(&absolute) && !queue.contains(&absolute) {
          queue.push_back(absolute);
        }
      } else if link_is_asset(&current_url, &link.href) {
        page_discovered_urls.push(absolute.clone());
        _register_edge(
          &mut edges,
          &current_url,
          &absolute,
          &page_title,
          &link.text,
          "links_to",
        );
        _register_row(
          &mut rows,
          &mut duplicates_skipped,
          &absolute,
          &link.text,
          &current_url,
          &page_title,
        );
        {
          let child_row = rows.get_mut(&absolute).unwrap();
          child_row.resource_kind = "asset".to_string();
          child_row.asset_kind = Some(classify_asset_kind(&absolute, None));
        }
        asset_urls.insert(absolute);
      } else if link_is_variable_page(&current_url, &link.href) {
        page_discovered_urls.push(absolute.clone());
        _register_edge(
          &mut edges,
          &current_url,
          &absolute,
          &page_title,
          &link.text,
          "links_to",
        );
        _register_row(
          &mut rows,
          &mut duplicates_skipped,
          &absolute,
          &link.text,
          &current_url,
          &page_title,
        );
        {
          let child_row = rows.get_mut(&absolute).unwrap();
          child_row.resource_kind = "variable_page".to_string();
        }
      }
    }

    {
      let row = rows.get_mut(&current_url).unwrap();
      let unique_discovered: HashSet<String> = page_discovered_urls.into_iter().collect();
      row.linked_documents = Some(unique_discovered.len());
    }

    let mut count_map = HashMap::new();
    count_map.insert("listing_pages".to_string(), listing_pages_fetched);
    count_map.insert("follow_pages".to_string(), follow_pages_fetched);
    count_map.insert("asset_probes".to_string(), asset_probes);
    count_map.insert("queued_pages".to_string(), queue.len());
    count_map.insert("rows".to_string(), rows.len());
    count_map.insert("edges".to_string(), edges.len());

    append_progress_event(
      config.progress_log_path.as_deref(),
      "inventory",
      "progress",
      "",
      &current_url,
      &row_kind,
      Some(page_result.status),
      Some(count_map),
      "",
    );
  }

  for url in asset_urls {
    if limit_reached(config.max_assets, asset_probes) {
      if let Some(ref progress) = progress_fn {
        progress(format!(
          "stopped asset-probe crawl at configured limit: {}",
          config.max_assets.unwrap()
        ));
      }
      break;
    }
    let mut row = rows.get_mut(&url).unwrap();
    if asset_probe_needed(row) {
      if config.request_delay_seconds > 0.0 {
        thread::sleep(Duration::from_secs_f64(config.request_delay_seconds));
      }
      let probe = probe_url_fn(&url, config.timeout_seconds, &config.user_agent)?;
      asset_probes += 1;
      update_probe_row(&mut row, &probe);

      let mut count_map = HashMap::new();
      count_map.insert("listing_pages".to_string(), listing_pages_fetched);
      count_map.insert("follow_pages".to_string(), follow_pages_fetched);
      count_map.insert("asset_probes".to_string(), asset_probes);
      count_map.insert("rows".to_string(), rows.len());
      count_map.insert("edges".to_string(), edges.len());

      append_progress_event(
        config.progress_log_path.as_deref(),
        "inventory",
        "progress",
        "",
        &url,
        "asset",
        Some(probe.status),
        Some(count_map),
        "",
      );
    }
  }

  let ordered_rows = sorted_rows(rows.into_values().collect());
  let ordered_edges = sorted_edges(edges.into_values().collect());

  let mut dead_links = Vec::new();
  let mut transient_links = Vec::new();
  for row in &ordered_rows {
    if row.link_state == "dead" {
      dead_links.push(row.clone());
    } else if row.link_state == "unknown"
      && row.http_status.is_some()
      && TRANSIENT_HTTP_STATUSES.contains(&row.http_status.unwrap())
    {
      transient_links.push(row.clone());
    }
  }

  let mut summary = HashMap::new();
  for row in &ordered_rows {
    *summary.entry(row.resource_kind.clone()).or_insert(0) += 1;
  }
  summary.insert("total_urls".to_string(), ordered_rows.len());
  summary.insert("total_edges".to_string(), ordered_edges.len());
  summary.insert("dead_links".to_string(), dead_links.len());
  summary.insert("transient_links".to_string(), transient_links.len());
  summary.insert("duplicates_skipped".to_string(), duplicates_skipped);

  let mut complete_counts = HashMap::new();
  complete_counts.insert("listing_pages".to_string(), listing_pages_fetched);
  complete_counts.insert("follow_pages".to_string(), follow_pages_fetched);
  complete_counts.insert("asset_probes".to_string(), asset_probes);
  complete_counts.insert("total_urls".to_string(), ordered_rows.len());
  complete_counts.insert("total_edges".to_string(), ordered_edges.len());
  complete_counts.insert("dead_links".to_string(), dead_links.len());
  complete_counts.insert("duplicates_skipped".to_string(), duplicates_skipped);

  append_progress_event(
    config.progress_log_path.as_deref(),
    "inventory",
    "complete",
    "inventory crawl complete",
    "",
    "",
    None,
    Some(complete_counts),
    "",
  );

  Ok(InventoryResult {
    rows: ordered_rows,
    edges: ordered_edges,
    summary,
    dead_links,
    duplicates_skipped,
  })
}

pub fn write_inventory_csv<P: AsRef<Path>>(rows: &[InventoryRow], path: P) -> Result<(), AppError> {
  if let Some(parent) = path.as_ref().parent() {
    let _ = fs::create_dir_all(parent);
  }
  let file = fs::File::create(path)
    .map_err(|e| AppError::RecordParseError(format!("failed to create inventory CSV: {e}")))?;
  let mut wtr = csv::Writer::from_writer(file);
  for row in rows {
    wtr
      .serialize(row)
      .map_err(|e| AppError::RecordParseError(format!("failed to serialize inventory row: {e}")))?;
  }
  wtr
    .flush()
    .map_err(|e| AppError::RecordParseError(format!("failed to flush inventory CSV: {e}")))?;
  Ok(())
}

pub fn write_inventory_edges_csv<P: AsRef<Path>>(
  edges: &[InventoryEdgeRow],
  path: P,
) -> Result<(), AppError> {
  if let Some(parent) = path.as_ref().parent() {
    let _ = fs::create_dir_all(parent);
  }
  let file = fs::File::create(path).map_err(|e| {
    AppError::RecordParseError(format!("failed to create inventory edges CSV: {e}"))
  })?;
  let mut wtr = csv::Writer::from_writer(file);
  for edge in edges {
    wtr.serialize(edge).map_err(|e| {
      AppError::RecordParseError(format!("failed to serialize inventory edge: {e}"))
    })?;
  }
  wtr
    .flush()
    .map_err(|e| AppError::RecordParseError(format!("failed to flush inventory edges CSV: {e}")))?;
  Ok(())
}

pub fn read_inventory_csv<P: AsRef<Path>>(path: P) -> Result<Vec<InventoryRow>, AppError> {
  let file = fs::File::open(path)
    .map_err(|e| AppError::RecordParseError(format!("failed to open inventory file: {e}")))?;
  let mut rdr = csv::Reader::from_reader(file);
  let mut records = Vec::new();
  for result in rdr.deserialize::<InventoryRow>() {
    let record = result.map_err(|e| {
      AppError::RecordParseError(format!("failed to deserialize inventory row: {e}"))
    })?;
    records.push(record);
  }
  Ok(records)
}

pub fn write_workspace_summary(
  result: &InventoryResult,
  config: &InventoryConfig,
) -> Result<PathBuf, AppError> {
  let _ = fs::create_dir_all(&config.workspace_dir);
  let summary_path = config.workspace_dir.join("02_source_inventory.md");

  let mut by_kind = HashMap::new();
  for row in &result.rows {
    *by_kind.entry(row.resource_kind.clone()).or_insert(0) += 1;
  }

  let mut by_asset_kind = HashMap::new();
  for row in &result.rows {
    if let Some(ref ak) = row.asset_kind {
      if !ak.is_empty() {
        *by_asset_kind.entry(ak.clone()).or_insert(0) += 1;
      }
    }
  }

  let mut transient_links = Vec::new();
  for row in &result.rows {
    if row.link_state == "unknown"
      && row.http_status.is_some()
      && TRANSIENT_HTTP_STATUSES.contains(&row.http_status.unwrap())
    {
      transient_links.push(row.clone());
    }
  }

  let listing_pages_count = result
    .rows
    .iter()
    .filter(|r| r.resource_kind == "listing_page")
    .count();

  let mut lines = vec![
    "# Source Inventory".to_string(),
    "".to_string(),
    format!("- Base URL: {}", config.base_url),
    format!("- Listing pages crawled: {listing_pages_count}"),
    format!("- Unique URLs: {}", result.rows.len()),
    format!("- Discovery edges: {}", result.edges.len()),
    format!("- Dead links: {}", result.dead_links.len()),
    format!("- Transient unresolved links: {}", transient_links.len()),
    format!("- Duplicate URLs skipped: {}", result.duplicates_skipped),
    "".to_string(),
    "## By Resource Kind".to_string(),
    "".to_string(),
    "| kind | count |".to_string(),
    "| --- | ---: |".to_string(),
  ];

  for kind in &[
    "listing_page",
    "dataset_page",
    "documentation_page",
    "variable_page",
    "asset",
    "other",
  ] {
    lines.push(format!("| {kind} | {} |", by_kind.get(*kind).unwrap_or(&0)));
  }

  lines.extend(vec![
    "".to_string(),
    "## By Asset Kind".to_string(),
    "".to_string(),
    "| kind | count |".to_string(),
    "| --- | ---: |".to_string(),
  ]);

  if !by_asset_kind.is_empty() {
    let mut sorted_asset_kinds: Vec<(String, usize)> = by_asset_kind.into_iter().collect();
    sorted_asset_kinds.sort_by(|a, b| a.0.cmp(&b.0));
    for (kind, count) in sorted_asset_kinds {
      lines.push(format!("| {kind} | {count} |"));
    }
  } else {
    lines.push("| none | 0 |".to_string());
  }

  lines.extend(vec![
    "".to_string(),
    "## Dead Links".to_string(),
    "".to_string(),
  ]);
  if !result.dead_links.is_empty() {
    lines.extend(vec![
      "| url | source | status | content_type |".to_string(),
      "| --- | --- | ---: | --- |".to_string(),
    ]);
    for row in result.dead_links.iter().take(25) {
      lines.push(format!(
        "| {} | {} | {} | {} |",
        row.url,
        row.source_url.as_deref().unwrap_or(""),
        row.http_status.map(|s| s.to_string()).unwrap_or_default(),
        row.content_type
      ));
    }
    if result.dead_links.len() > 25 {
      lines.push(format!(
        "\n- Additional dead links omitted: {}",
        result.dead_links.len() - 25
      ));
    }
  } else {
    lines.push("- None".to_string());
  }

  lines.extend(vec![
    "".to_string(),
    "## Transient Unresolved Links".to_string(),
    "".to_string(),
  ]);
  if !transient_links.is_empty() {
    lines.extend(vec![
      "| url | source | status | content_type |".to_string(),
      "| --- | --- | ---: | --- |".to_string(),
    ]);
    for row in transient_links.iter().take(25) {
      lines.push(format!(
        "| {} | {} | {} | {} |",
        row.url,
        row.source_url.as_deref().unwrap_or(""),
        row.http_status.map(|s| s.to_string()).unwrap_or_default(),
        row.content_type
      ));
    }
    if transient_links.len() > 25 {
      lines.push(format!(
        "\n- Additional transient unresolved links omitted: {}",
        transient_links.len() - 25
      ));
    }
  } else {
    lines.push("- None".to_string());
  }

  fs::write(&summary_path, lines.join("\n") + "\n")
    .map_err(|e| AppError::RecordParseError(format!("failed to write inventory summary: {e}")))?;

  Ok(summary_path)
}

pub fn run_inventory(config: &InventoryConfig) -> Result<(InventoryResult, PathBuf), AppError> {
  let result = crawl_inventory(
    config,
    fetch_html_ureq,
    probe_url_ureq,
    Some(|msg| eprintln!("{msg}")),
  )?;
  write_inventory_csv(&result.rows, &config.output_path)?;
  write_inventory_edges_csv(&result.edges, &config.edge_output_path)?;
  let summary_path = write_workspace_summary(&result, config)?;
  Ok((result, summary_path))
}
