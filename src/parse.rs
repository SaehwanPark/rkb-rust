//! Document parsing and text chunking engine.

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
  clippy::too_many_arguments,
  clippy::case_sensitive_file_extension_comparisons,
  clippy::collapsible_else_if,
  clippy::must_use_candidate,
  clippy::implicit_saturating_sub,
  clippy::needless_range_loop
)]

use crate::config::ParsingConfig;
use crate::error::AppError;
use crate::records::{ChunkMetadata, DatasetMetadataRow, DocumentMetadataRow};
use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

/// Structured failure logged when a resource fails parsing or validation.
#[derive(Clone, Debug)]
pub struct ParsingFailure {
  pub url: String,
  pub local_path: String,
  pub reason: String,
}

/// Compilation metrics from the parsing pipeline.
pub struct ParsingResult {
  pub config: ParsingConfig,
  pub parsed_datasets_count: usize,
  pub parsed_documents_count: usize,
  pub chunks_count: usize,
  pub failures: Vec<ParsingFailure>,
}

fn normalize_text(input: &str) -> String {
  let mut temp = String::new();
  let mut last_was_space = false;

  // Phase 1: normalize horizontal spaces [ \t]+ -> " "
  for c in input.chars() {
    if c == ' ' || c == '\t' {
      if !last_was_space {
        temp.push(' ');
        last_was_space = true;
      }
    } else {
      if c == '\n' || c == '\r' {
        last_was_space = false;
        temp.push(c);
      } else {
        temp.push(c);
        last_was_space = false;
      }
    }
  }

  // Phase 2: Collapse \n\s*\n+ to \n\n.
  let lines: Vec<&str> = temp.split('\n').collect();
  let mut final_lines = Vec::new();
  let mut consecutive_empty_lines = 0;

  for line in lines {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      consecutive_empty_lines += 1;
    } else {
      if consecutive_empty_lines > 0 {
        final_lines.push("");
        consecutive_empty_lines = 0;
      }
      final_lines.push(line);
    }
  }

  final_lines.join("\n").trim().to_string()
}

/// Slices text into overlapping windows matching exact boundary heuristics.
pub fn chunk_text(text: &str, chunk_size: usize, chunk_overlap: usize) -> Vec<String> {
  if chunk_size == 0 || chunk_overlap >= chunk_size {
    return Vec::new();
  }

  let normalized = normalize_text(text);
  if normalized.is_empty() {
    return Vec::new();
  }

  let chars: Vec<char> = normalized.chars().collect();
  let text_len = chars.len();

  if text_len <= chunk_size {
    return vec![normalized];
  }

  let mut chunks = Vec::new();
  let mut start = 0;

  while start < text_len {
    if start + chunk_size >= text_len {
      let chunk_str: String = chars[start..].iter().collect();
      let trimmed = chunk_str.trim().to_string();
      if !trimmed.is_empty() {
        chunks.push(trimmed);
      }
      break;
    }

    let mut end = start + chunk_size;

    // Look back for a word boundary (space) near the end to avoid cutting words
    let search_start = if end > 30 {
      std::cmp::max(start, end - 30)
    } else {
      start
    };
    let mut space_idx = None;
    for idx in (search_start..end).rev() {
      if chars[idx] == ' ' {
        space_idx = Some(idx);
        break;
      }
    }

    if let Some(s_idx) = space_idx {
      if s_idx > start {
        end = s_idx;
      }
    }

    let chunk_str: String = chars[start..end].iter().collect();
    let trimmed = chunk_str.trim().to_string();
    if !trimmed.is_empty() {
      chunks.push(trimmed);
    }

    // Align the start of the next overlapping chunk to the nearest word boundary
    let mut next_start = if end > chunk_overlap {
      end - chunk_overlap
    } else {
      0
    };

    if next_start < text_len {
      let search_overlap_start = if next_start > 15 {
        std::cmp::max(start, next_start - 15)
      } else {
        start
      };
      let search_overlap_end = std::cmp::min(text_len, next_start + 15);
      let mut space_overlap_idx = None;
      for idx in search_overlap_start..search_overlap_end {
        if chars[idx] == ' ' {
          space_overlap_idx = Some(idx);
          break;
        }
      }

      if let Some(so_idx) = space_overlap_idx {
        if so_idx > start {
          next_start = so_idx + 1;
        }
      }
    }

    next_start = std::cmp::min(next_start, end);

    if next_start <= start {
      start = end;
    } else {
      start = next_start;
    }
  }

  chunks.into_iter().filter(|c| !c.is_empty()).collect()
}

fn walk_nodes(node: ego_tree::NodeRef<'_, scraper::Node>, out: &mut String) {
  match node.value() {
    scraper::Node::Text(text) => {
      out.push_str(text);
    }
    scraper::Node::Element(el) => {
      let name = el.name();
      if name == "head" || name == "script" || name == "style" || name == "nav" || name == "footer"
      {
        return;
      }
      let is_block = matches!(
        name,
        "p"
          | "div"
          | "h1"
          | "h2"
          | "h3"
          | "h4"
          | "h5"
          | "h6"
          | "li"
          | "tr"
          | "br"
          | "ul"
          | "ol"
          | "table"
          | "tbody"
          | "thead"
          | "section"
          | "article"
          | "aside"
          | "header"
          | "main"
      );
      if is_block {
        if name == "br" {
          out.push('\n');
        } else {
          if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
          }
          for child in node.children() {
            walk_nodes(child, out);
          }
          if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
          }
        }
      } else {
        for child in node.children() {
          walk_nodes(child, out);
        }
      }
    }
    _ => {
      for child in node.children() {
        walk_nodes(child, out);
      }
    }
  }
}

/// Extracts clean body text from HTML document.
pub fn parse_html(local_path: &Path) -> Result<String, AppError> {
  let file_content = fs::read_to_string(local_path)
    .map_err(|e| AppError::RecordParseError(format!("failed to read HTML file: {e}")))?;
  let doc = scraper::Html::parse_document(&file_content);
  let mut extracted = String::new();
  walk_nodes(doc.tree.root(), &mut extracted);
  Ok(extracted)
}

/// Extracts page-by-page text from PDF file.
pub fn parse_pdf(local_path: &Path) -> Result<Vec<(usize, String)>, AppError> {
  let page_texts = pdf_extract::extract_text_by_pages(local_path)
    .map_err(|e| AppError::RecordParseError(format!("failed to extract PDF text: {e}")))?;
  let mut pages = Vec::new();
  for (idx, text) in page_texts.into_iter().enumerate() {
    pages.push((idx + 1, text));
  }
  Ok(pages)
}

fn sheet_sort_key(name: &str) -> (usize, String) {
  if let Some(start) = name.find("sheet") {
    if let Some(end) = name.find(".xml") {
      if start + 5 < end {
        let num_str = &name[start + 5..end];
        if let Ok(num) = num_str.parse::<usize>() {
          return (num, name.to_string());
        }
      }
    }
  }
  (0, name.to_string())
}

fn excel_column_to_index(col: &str) -> usize {
  let mut index = 0;
  for c in col.chars() {
    if c.is_ascii_alphabetic() {
      let val = (c.to_ascii_uppercase() as usize) - ('A' as usize) + 1;
      index = index * 26 + val;
    }
  }
  if index > 0 { index - 1 } else { 0 }
}

fn extract_column_letters(r: &str) -> &str {
  let end = r
    .find(|c: char| !c.is_ascii_alphabetic())
    .unwrap_or(r.len());
  &r[..end]
}

fn read_xlsx_shared_strings<R: std::io::Read>(
  reader: &mut quick_xml::Reader<BufReader<R>>,
) -> Result<Vec<String>, AppError> {
  use quick_xml::events::Event;
  let mut shared_strings = Vec::new();
  let mut current_string = String::new();
  let mut in_si = false;
  let mut buf = Vec::new();

  loop {
    match reader.read_event_into(&mut buf) {
      Ok(Event::Start(ref e)) if e.local_name().as_ref() == b"si" => {
        in_si = true;
        current_string.clear();
      }
      Ok(Event::End(ref e)) if e.local_name().as_ref() == b"si" => {
        in_si = false;
        shared_strings.push(current_string.trim().to_string());
      }
      Ok(Event::Text(ref e)) if in_si => {
        let decoded = reader
          .decoder()
          .decode(e.as_ref())
          .map_err(|e| AppError::RecordParseError(format!("XML decode error: {e}")))?;
        let unescaped = quick_xml::escape::unescape(&decoded)
          .map_err(|e| AppError::RecordParseError(format!("XML unescape error: {e}")))?;
        current_string.push_str(&unescaped);
      }
      Ok(Event::Eof) => break,
      Err(e) => return Err(AppError::RecordParseError(format!("XML error: {e}"))),
      _ => {}
    }
    buf.clear();
  }
  Ok(shared_strings)
}

fn parse_worksheet_xml<R: std::io::Read>(
  reader: &mut quick_xml::Reader<BufReader<R>>,
  shared_strings: &[String],
) -> Result<String, AppError> {
  use quick_xml::events::Event;
  let mut rows = Vec::new();
  let mut current_row = Vec::new();
  let mut current_cell_value = String::new();
  let mut current_cell_type = String::new();
  let mut current_cell_ref = String::new();
  let mut in_v = false;
  let mut in_is_t = false;
  let mut in_is = false;
  let mut buf = Vec::new();

  loop {
    match reader.read_event_into(&mut buf) {
      Ok(Event::Start(ref e)) => {
        let name = e.local_name();
        let name_bytes = name.as_ref();
        if name_bytes == b"row" {
          current_row.clear();
        } else if name_bytes == b"c" {
          current_cell_value.clear();
          current_cell_type.clear();
          current_cell_ref.clear();
          for attr in e.attributes().flatten() {
            let key = attr.key.local_name();
            let key_bytes = key.as_ref();
            if key_bytes == b"t" {
              #[allow(deprecated)]
              if let Ok(val) = attr.unescape_value() {
                current_cell_type = val.into_owned();
              }
            } else if key_bytes == b"r" {
              #[allow(deprecated)]
              if let Ok(val) = attr.unescape_value() {
                current_cell_ref = val.into_owned();
              }
            }
          }
        } else if name_bytes == b"v" {
          in_v = true;
        } else if name_bytes == b"is" {
          in_is = true;
        } else if name_bytes == b"t" && in_is {
          in_is_t = true;
        }
      }
      Ok(Event::End(ref e)) => {
        let name = e.local_name();
        let name_bytes = name.as_ref();
        if name_bytes == b"row" {
          let row_text = current_row.join("\t");
          if !row_text.trim().is_empty() {
            rows.push(row_text);
          }
        } else if name_bytes == b"c" {
          let cell_text = if current_cell_type == "s" {
            if let Ok(idx) = current_cell_value.parse::<usize>() {
              shared_strings
                .get(idx)
                .cloned()
                .unwrap_or_else(|| current_cell_value.clone())
            } else {
              current_cell_value.clone()
            }
          } else {
            current_cell_value.clone()
          };

          let cell_text_trimmed = cell_text.trim().to_string();
          if !current_cell_ref.is_empty() {
            let col_letters = extract_column_letters(&current_cell_ref);
            let col_idx = excel_column_to_index(col_letters);
            while current_row.len() < col_idx {
              current_row.push(String::new());
            }
          }
          current_row.push(cell_text_trimmed);
        } else if name_bytes == b"v" {
          in_v = false;
        } else if name_bytes == b"is" {
          in_is = false;
        } else if name_bytes == b"t" && in_is {
          in_is_t = false;
        }
      }
      Ok(Event::Text(ref e)) => {
        if in_v || in_is_t {
          let decoded = reader
            .decoder()
            .decode(e.as_ref())
            .map_err(|e| AppError::RecordParseError(format!("XML decode error: {e}")))?;
          let unescaped = quick_xml::escape::unescape(&decoded)
            .map_err(|e| AppError::RecordParseError(format!("XML unescape error: {e}")))?;
          current_cell_value.push_str(&unescaped);
        }
      }
      Ok(Event::Eof) => break,
      Err(e) => return Err(AppError::RecordParseError(format!("XML error: {e}"))),
      _ => {}
    }
    buf.clear();
  }
  Ok(rows.join("\n"))
}

/// Extracts text page-by-page from XLSX worksheet structure.
pub fn parse_xlsx(local_path: &Path) -> Result<Vec<(usize, String)>, AppError> {
  let file = File::open(local_path)
    .map_err(|e| AppError::RecordParseError(format!("failed to open XLSX zip file: {e}")))?;
  let mut archive = zip::ZipArchive::new(file)
    .map_err(|e| AppError::RecordParseError(format!("invalid ZIP file format: {e}")))?;

  let mut shared_strings = Vec::new();
  if let Ok(shared_strings_file) = archive.by_name("xl/sharedStrings.xml") {
    let mut reader = quick_xml::Reader::from_reader(BufReader::new(shared_strings_file));
    shared_strings = read_xlsx_shared_strings(&mut reader)?;
  }

  let mut sheet_names = Vec::new();
  for idx in 0..archive.len() {
    if let Ok(file) = archive.by_index(idx) {
      let name = file.name();
      if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
        sheet_names.push(name.to_string());
      }
    }
  }
  sheet_names.sort_by_key(|n| sheet_sort_key(n));

  let mut sheets = Vec::new();
  for (idx, name) in sheet_names.into_iter().enumerate() {
    let sheet_file = archive
      .by_name(&name)
      .map_err(|e| AppError::RecordParseError(format!("failed to extract worksheet XML: {e}")))?;
    let mut reader = quick_xml::Reader::from_reader(BufReader::new(sheet_file));
    let sheet_text = parse_worksheet_xml(&mut reader, &shared_strings)?;
    sheets.push((idx + 1, sheet_text));
  }

  Ok(sheets)
}

fn is_safe_output_id(value: &str) -> bool {
  if value.is_empty() {
    return false;
  }
  let mut chars = value.chars();
  if let Some(first) = chars.next() {
    if !first.is_ascii_alphanumeric() {
      return false;
    }
  } else {
    return false;
  }
  for c in chars {
    if !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '-' {
      return false;
    }
  }
  true
}

fn safe_output_id_error(field: &str, value: &str) -> Option<String> {
  if value.trim().is_empty() {
    return Some(format!("{field} must not be empty"));
  }
  let path = Path::new(value);
  if value != path.file_name().and_then(|s| s.to_str()).unwrap_or("")
    || path.is_absolute()
    || value.contains("..")
    || !is_safe_output_id(value)
  {
    return Some(format!(
      "{field} contains unsafe output path characters: {value}"
    ));
  }
  None
}

fn read_datasets_csv(path: &Path) -> Result<Vec<DatasetMetadataRow>, AppError> {
  let file = File::open(path).map_err(|e| {
    AppError::RecordParseError(format!(
      "failed to open datasets csv at {}: {e}",
      path.display()
    ))
  })?;
  let mut reader = csv::Reader::from_reader(file);
  let mut rows = Vec::new();
  for result in reader.deserialize() {
    let row: DatasetMetadataRow = result
      .map_err(|e| AppError::RecordParseError(format!("failed to parse dataset csv row: {e}")))?;
    rows.push(row);
  }
  Ok(rows)
}

fn read_documents_csv(path: &Path) -> Result<Vec<DocumentMetadataRow>, AppError> {
  let file = File::open(path).map_err(|e| {
    AppError::RecordParseError(format!(
      "failed to open documents csv at {}: {e}",
      path.display()
    ))
  })?;
  let mut reader = csv::Reader::from_reader(file);
  let mut rows = Vec::new();
  for result in reader.deserialize() {
    let row: DocumentMetadataRow = result
      .map_err(|e| AppError::RecordParseError(format!("failed to parse document csv row: {e}")))?;
    rows.push(row);
  }
  Ok(rows)
}

fn write_parsing_workspace_summary(result: &ParsingResult) -> Result<PathBuf, AppError> {
  fs::create_dir_all(&result.config.workspace_dir)
    .map_err(|e| AppError::RecordParseError(format!("failed to create workspace dir: {e}")))?;
  let summary_path = result.config.workspace_dir.join("05_parsing_pack.md");
  let mut file = File::create(&summary_path)
    .map_err(|e| AppError::RecordParseError(format!("failed to create summary pack: {e}")))?;

  writeln!(file, "# Parsing Pack").unwrap();
  writeln!(file).unwrap();
  writeln!(
    file,
    "- Datasets metadata path: {}",
    result.config.datasets_metadata_path.display()
  )
  .unwrap();
  writeln!(
    file,
    "- Documents metadata path: {}",
    result.config.documents_metadata_path.display()
  )
  .unwrap();
  writeln!(file, "- Datasets parsed: {}", result.parsed_datasets_count).unwrap();
  writeln!(
    file,
    "- Documents parsed: {}",
    result.parsed_documents_count
  )
  .unwrap();
  writeln!(file, "- Chunks generated: {}", result.chunks_count).unwrap();
  writeln!(file, "- Failures: {}", result.failures.len()).unwrap();
  writeln!(file).unwrap();
  writeln!(file, "## Outputs").unwrap();
  writeln!(file).unwrap();
  writeln!(
    file,
    "- Parsed HTML directory: {}",
    result.config.parsed_root.join("html").display()
  )
  .unwrap();
  writeln!(
    file,
    "- Parsed PDF directory: {}",
    result.config.parsed_root.join("pdf").display()
  )
  .unwrap();
  writeln!(
    file,
    "- Parsed XLSX directory: {}",
    result.config.parsed_root.join("xlsx").display()
  )
  .unwrap();
  writeln!(
    file,
    "- Chunks directory: {}",
    result.config.parsed_root.join("chunks").display()
  )
  .unwrap();
  writeln!(
    file,
    "- Unified chunks file: {}",
    result.config.parsed_root.join("chunks.jsonl").display()
  )
  .unwrap();
  writeln!(file).unwrap();
  writeln!(file, "## Failures").unwrap();
  writeln!(file).unwrap();

  if result.failures.is_empty() {
    writeln!(file, "- None").unwrap();
  } else {
    writeln!(file, "| url | local_path | reason |").unwrap();
    writeln!(file, "| --- | --- | --- |").unwrap();
    for failure in result.failures.iter().take(25) {
      let safe_reason = failure.reason.replace('|', "\\|").replace('\n', " ");
      writeln!(
        file,
        "| {} | {} | {} |",
        failure.url, failure.local_path, safe_reason
      )
      .unwrap();
    }
    if result.failures.len() > 25 {
      writeln!(file).unwrap();
      writeln!(
        file,
        "- Additional failures omitted: {}",
        result.failures.len() - 25
      )
      .unwrap();
    }
  }

  Ok(summary_path)
}

/// Main entry point executing Document Parsing and Text Chunking.
pub fn run_parsing(config: &ParsingConfig) -> Result<(), AppError> {
  let datasets = read_datasets_csv(&config.datasets_metadata_path)?;
  let documents = read_documents_csv(&config.documents_metadata_path)?;

  let html_out = config.parsed_root.join("html");
  let pdf_out = config.parsed_root.join("pdf");
  let xlsx_out = config.parsed_root.join("xlsx");
  let chunks_out = config.parsed_root.join("chunks");

  for path in &[&html_out, &pdf_out, &xlsx_out, &chunks_out] {
    if path.exists() {
      fs::remove_dir_all(path)
        .map_err(|e| AppError::RecordParseError(format!("failed to clean output path: {e}")))?;
    }
    fs::create_dir_all(path)
      .map_err(|e| AppError::RecordParseError(format!("failed to create output path: {e}")))?;
  }

  let mut parsed_datasets_count = 0;
  let mut parsed_documents_count = 0;
  let mut chunks_count = 0;
  let mut failures = Vec::new();

  let valid_dataset_ids: std::collections::HashSet<String> =
    datasets.iter().map(|d| d.dataset_id.clone()).collect();

  let jsonl_path = config.parsed_root.join("chunks.jsonl");
  let mut jsonl_file = File::create(&jsonl_path).map_err(|e| {
    AppError::RecordParseError(format!("failed to create chunks jsonl stream: {e}"))
  })?;

  // 1. Process Datasets (all are HTML files)
  for dataset in &datasets {
    if let Some(err) = safe_output_id_error("dataset_id", &dataset.dataset_id) {
      failures.push(ParsingFailure {
        url: dataset.source_url.clone(),
        local_path: dataset.local_path.clone(),
        reason: err,
      });
      continue;
    }

    let local_path_str = dataset.local_path.trim();
    if local_path_str.is_empty() {
      failures.push(ParsingFailure {
        url: dataset.source_url.clone(),
        local_path: String::new(),
        reason: "dataset has empty local path".to_string(),
      });
      continue;
    }

    let local_path = Path::new(local_path_str);
    if !local_path.is_file() {
      failures.push(ParsingFailure {
        url: dataset.source_url.clone(),
        local_path: local_path_str.to_string(),
        reason: "dataset file does not exist locally".to_string(),
      });
      continue;
    }

    match parse_html(local_path) {
      Ok(text) => {
        if text.trim().is_empty() {
          failures.push(ParsingFailure {
            url: dataset.source_url.clone(),
            local_path: local_path_str.to_string(),
            reason: "extracted HTML text is empty".to_string(),
          });
          continue;
        }

        // Save raw clean text
        let txt_path = html_out.join(format!("{}.txt", dataset.dataset_id));
        if let Err(e) = fs::write(&txt_path, &text) {
          failures.push(ParsingFailure {
            url: dataset.source_url.clone(),
            local_path: local_path_str.to_string(),
            reason: format!("failed to write raw text: {e}"),
          });
          continue;
        }
        parsed_datasets_count += 1;

        // Slice into chunks
        let txt_chunks = chunk_text(&text, config.chunk_size, config.chunk_overlap);
        for (idx, chunk_txt) in txt_chunks.into_iter().enumerate() {
          let chunk_id = format!("{}__chunk_{}", dataset.dataset_id, idx);
          let chunk = ChunkMetadata {
            chunk_id: chunk_id.clone(),
            source_document: dataset.local_path.clone(),
            page: None,
            text: chunk_txt,
            dataset: dataset.dataset_id.clone(),
            url: dataset.source_url.clone(),
          };

          // Save individual chunk JSON
          let chunk_path = chunks_out.join(format!("{}.json", chunk_id));
          let json_str = serde_json::to_string_pretty(&chunk)
            .map_err(|e| AppError::RecordParseError(format!("failed to serialize chunk: {e}")))?;
          fs::write(&chunk_path, json_str)
            .map_err(|e| AppError::RecordParseError(format!("failed to write chunk file: {e}")))?;

          // Write to jsonl
          let json_str = serde_json::to_string(&chunk)
            .map_err(|e| AppError::RecordParseError(format!("failed to serialize chunk: {e}")))?;
          writeln!(jsonl_file, "{}", json_str)
            .map_err(|e| AppError::RecordParseError(format!("failed to write to jsonl: {e}")))?;
          chunks_count += 1;
        }
      }
      Err(e) => {
        failures.push(ParsingFailure {
          url: dataset.source_url.clone(),
          local_path: local_path_str.to_string(),
          reason: format!("failed to parse/chunk dataset page: {e}"),
        });
      }
    }
  }

  // 2. Process Documents (HTML, PDF, or XLSX)
  for doc in &documents {
    if let Some(err) = safe_output_id_error("document_id", &doc.document_id) {
      failures.push(ParsingFailure {
        url: doc.source_url.clone(),
        local_path: doc.local_path.clone(),
        reason: err,
      });
      continue;
    }

    if let Some(err) = safe_output_id_error("dataset_id", &doc.dataset_id) {
      failures.push(ParsingFailure {
        url: doc.source_url.clone(),
        local_path: doc.local_path.clone(),
        reason: err,
      });
      continue;
    }

    if !valid_dataset_ids.contains(&doc.dataset_id) {
      failures.push(ParsingFailure {
        url: doc.source_url.clone(),
        local_path: doc.local_path.clone(),
        reason: format!(
          "document dataset_id '{}' not found in datasets metadata",
          doc.dataset_id
        ),
      });
      continue;
    }

    let local_path_str = doc.local_path.trim();
    if local_path_str.is_empty() {
      failures.push(ParsingFailure {
        url: doc.source_url.clone(),
        local_path: String::new(),
        reason: "document has empty local path".to_string(),
      });
      continue;
    }

    let local_path = Path::new(local_path_str);
    if !local_path.is_file() {
      failures.push(ParsingFailure {
        url: doc.source_url.clone(),
        local_path: local_path_str.to_string(),
        reason: "document file does not exist locally".to_string(),
      });
      continue;
    }

    let kind = doc.document_kind.to_lowercase();
    if kind != "pdf" && kind != "html" && kind != "xlsx" {
      failures.push(ParsingFailure {
        url: doc.source_url.clone(),
        local_path: local_path_str.to_string(),
        reason: format!("unsupported document kind: {}", doc.document_kind),
      });
      continue;
    }

    if kind == "pdf" {
      match parse_pdf(local_path) {
        Ok(pages) => {
          let combined_text: String = pages
            .iter()
            .map(|(_, t)| t.clone())
            .collect::<Vec<_>>()
            .join("\n\n");
          if combined_text.trim().is_empty() {
            failures.push(ParsingFailure {
              url: doc.source_url.clone(),
              local_path: local_path_str.to_string(),
              reason:
                "extracted PDF text is empty (PDF may contain only scanned images and require OCR)"
                  .to_string(),
            });
            continue;
          }

          let txt_path = pdf_out.join(format!("{}.txt", doc.document_id));
          fs::write(&txt_path, &combined_text).map_err(|e| {
            AppError::RecordParseError(format!("failed to write PDF text file: {e}"))
          })?;
          parsed_documents_count += 1;

          for (page_num, page_text) in pages {
            if page_text.trim().is_empty() {
              continue;
            }
            let page_chunks = chunk_text(&page_text, config.chunk_size, config.chunk_overlap);
            for (idx, chunk_txt) in page_chunks.into_iter().enumerate() {
              let chunk_id = format!("{}__p{}__chunk_{}", doc.document_id, page_num, idx);
              let chunk = ChunkMetadata {
                chunk_id: chunk_id.clone(),
                source_document: doc.local_path.clone(),
                page: Some(page_num),
                text: chunk_txt,
                dataset: doc.dataset_id.clone(),
                url: doc.source_url.clone(),
              };

              let chunk_path = chunks_out.join(format!("{}.json", chunk_id));
              let json_str = serde_json::to_string_pretty(&chunk).map_err(|e| {
                AppError::RecordParseError(format!("failed to serialize chunk: {e}"))
              })?;
              fs::write(&chunk_path, json_str).map_err(|e| {
                AppError::RecordParseError(format!("failed to write chunk file: {e}"))
              })?;

              let json_str = serde_json::to_string(&chunk).map_err(|e| {
                AppError::RecordParseError(format!("failed to serialize chunk: {e}"))
              })?;
              writeln!(jsonl_file, "{}", json_str).map_err(|e| {
                AppError::RecordParseError(format!("failed to write to jsonl: {e}"))
              })?;
              chunks_count += 1;
            }
          }
        }
        Err(e) => {
          failures.push(ParsingFailure {
            url: doc.source_url.clone(),
            local_path: local_path_str.to_string(),
            reason: format!("failed to parse/chunk document: {e}"),
          });
        }
      }
    } else if kind == "html" {
      match parse_html(local_path) {
        Ok(text) => {
          if text.trim().is_empty() {
            failures.push(ParsingFailure {
              url: doc.source_url.clone(),
              local_path: local_path_str.to_string(),
              reason: "extracted HTML text is empty".to_string(),
            });
            continue;
          }

          let txt_path = html_out.join(format!("{}.txt", doc.document_id));
          fs::write(&txt_path, &text).map_err(|e| {
            AppError::RecordParseError(format!("failed to write HTML text file: {e}"))
          })?;
          parsed_documents_count += 1;

          let txt_chunks = chunk_text(&text, config.chunk_size, config.chunk_overlap);
          for (idx, chunk_txt) in txt_chunks.into_iter().enumerate() {
            let chunk_id = format!("{}__chunk_{}", doc.document_id, idx);
            let chunk = ChunkMetadata {
              chunk_id: chunk_id.clone(),
              source_document: doc.local_path.clone(),
              page: None,
              text: chunk_txt,
              dataset: doc.dataset_id.clone(),
              url: doc.source_url.clone(),
            };

            let chunk_path = chunks_out.join(format!("{}.json", chunk_id));
            let json_str = serde_json::to_string_pretty(&chunk)
              .map_err(|e| AppError::RecordParseError(format!("failed to serialize chunk: {e}")))?;
            fs::write(&chunk_path, json_str).map_err(|e| {
              AppError::RecordParseError(format!("failed to write chunk file: {e}"))
            })?;

            let json_str = serde_json::to_string(&chunk)
              .map_err(|e| AppError::RecordParseError(format!("failed to serialize chunk: {e}")))?;
            writeln!(jsonl_file, "{}", json_str)
              .map_err(|e| AppError::RecordParseError(format!("failed to write to jsonl: {e}")))?;
            chunks_count += 1;
          }
        }
        Err(e) => {
          failures.push(ParsingFailure {
            url: doc.source_url.clone(),
            local_path: local_path_str.to_string(),
            reason: format!("failed to parse/chunk document: {e}"),
          });
        }
      }
    } else if kind == "xlsx" {
      match parse_xlsx(local_path) {
        Ok(sheets) => {
          let combined_text: String = sheets
            .iter()
            .map(|(_, t)| t.clone())
            .collect::<Vec<_>>()
            .join("\n\n");
          if combined_text.trim().is_empty() {
            failures.push(ParsingFailure {
              url: doc.source_url.clone(),
              local_path: local_path_str.to_string(),
              reason: "extracted XLSX text is empty".to_string(),
            });
            continue;
          }

          let txt_path = xlsx_out.join(format!("{}.txt", doc.document_id));
          fs::write(&txt_path, &combined_text).map_err(|e| {
            AppError::RecordParseError(format!("failed to write XLSX text file: {e}"))
          })?;
          parsed_documents_count += 1;

          for (sheet_num, sheet_text) in sheets {
            if sheet_text.trim().is_empty() {
              continue;
            }
            let sheet_chunks = chunk_text(&sheet_text, config.chunk_size, config.chunk_overlap);
            for (idx, chunk_txt) in sheet_chunks.into_iter().enumerate() {
              let chunk_id = format!("{}__s{}__chunk_{}", doc.document_id, sheet_num, idx);
              let chunk = ChunkMetadata {
                chunk_id: chunk_id.clone(),
                source_document: doc.local_path.clone(),
                page: Some(sheet_num),
                text: chunk_txt,
                dataset: doc.dataset_id.clone(),
                url: doc.source_url.clone(),
              };

              let chunk_path = chunks_out.join(format!("{}.json", chunk_id));
              let json_str = serde_json::to_string_pretty(&chunk).map_err(|e| {
                AppError::RecordParseError(format!("failed to serialize chunk: {e}"))
              })?;
              fs::write(&chunk_path, json_str).map_err(|e| {
                AppError::RecordParseError(format!("failed to write chunk file: {e}"))
              })?;

              let json_str = serde_json::to_string(&chunk).map_err(|e| {
                AppError::RecordParseError(format!("failed to serialize chunk: {e}"))
              })?;
              writeln!(jsonl_file, "{}", json_str).map_err(|e| {
                AppError::RecordParseError(format!("failed to write to jsonl: {e}"))
              })?;
              chunks_count += 1;
            }
          }
        }
        Err(e) => {
          failures.push(ParsingFailure {
            url: doc.source_url.clone(),
            local_path: local_path_str.to_string(),
            reason: format!("failed to parse/chunk document: {e}"),
          });
        }
      }
    }
  }

  let result = ParsingResult {
    config: config.clone(),
    parsed_datasets_count,
    parsed_documents_count,
    chunks_count,
    failures,
  };

  write_parsing_workspace_summary(&result)?;

  if !result.failures.is_empty() {
    return Err(AppError::RecordParseError(format!(
      "parsing completed with {} failures; see workspace summary pack",
      result.failures.len()
    )));
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_normalize_text() {
    let input = "hello \t  world \n\n \n new \r\n line";
    let got = normalize_text(input);
    assert_eq!(got, "hello world \n\n new \r\n line");
  }

  #[test]
  fn test_chunk_text_simple() {
    let input = "In 2017, the Innovation Center launched the Accountable Health Communities Model.";
    let chunks = chunk_text(input, 30, 5);
    // Boundary space resolution should keep words intact
    assert!(!chunks.is_empty());
    for c in &chunks {
      assert!(c.len() <= 30);
    }
  }

  #[test]
  fn test_excel_column_converters() {
    assert_eq!(extract_column_letters("A1"), "A");
    assert_eq!(extract_column_letters("AB12"), "AB");
    assert_eq!(excel_column_to_index("A"), 0);
    assert_eq!(excel_column_to_index("B"), 1);
    assert_eq!(excel_column_to_index("Z"), 25);
    assert_eq!(excel_column_to_index("AA"), 26);
    assert_eq!(excel_column_to_index("AB"), 27);
  }

  #[test]
  fn test_html_block_newlines() {
    let html = "<div>Hello</div><p>World</p>";
    let doc = scraper::Html::parse_document(html);
    let mut out = String::new();
    walk_nodes(doc.tree.root(), &mut out);
    let normalized = normalize_text(&out);
    assert_eq!(normalized, "Hello\nWorld");
  }
}
