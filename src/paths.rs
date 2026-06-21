//! Path resolution helper functions.

use std::path::{Path, PathBuf};

/// Resolves the relative path fallback to a file packaged inside the knowledge base data,
/// falling back to local `data/`.
#[must_use]
pub fn get_packaged_data_path(subpath: &str) -> PathBuf {
  // In development/test environments, fallback to the local data directory.
  Path::new("data").join(subpath)
}
