//! Vector store. M0 stub: embedding metadata lives in SQLite, vectors are
//! not persisted, and `vector_search` returns no hits. Lance replaces this
//! in M1 behind the same [`crate::Storage`] trait.

use std::path::{Path, PathBuf};

use crate::error::Result;

/// Placeholder for `vectors/<model>.lance`.
#[derive(Debug, Clone)]
pub struct VectorStore {
    root: PathBuf,
}

impl VectorStore {
    /// Store rooted at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Ensure the directory exists.
    pub fn init(&self) -> Result<()> {
        std::fs::create_dir_all(&self.root)?;
        Ok(())
    }

    /// Whether any vectors are stored. Always false in M0.
    pub fn is_empty(&self) -> bool {
        true
    }
}
