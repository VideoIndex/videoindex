//! `manifest.json`: the one file a reader parses before deciding whether it
//! can open an index.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use vi_core::IndexId;

use crate::error::{IndexError, Result};

/// Current on-disk schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// File name inside the index directory.
pub const MANIFEST_FILE: &str = "manifest.json";

/// Hash and size of one file in the index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileHash {
    /// blake3 hex.
    pub blake3: String,
    /// Bytes.
    pub size: u64,
}

/// The manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Schema version.
    pub schema_version: u32,
    /// `videoindex <version>`.
    pub created_by: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Last manifest write.
    pub updated_at: DateTime<Utc>,
    /// Stable index id.
    pub index_id: IndexId,
    /// Content hashes of `meta.sqlite` and vector files, refreshed on
    /// `compact`.
    pub files: BTreeMap<String, FileHash>,
    /// Optional detached signature over `files` (hosted use).
    pub signature: Option<String>,
}

impl Manifest {
    /// A new manifest for an empty index.
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            schema_version: SCHEMA_VERSION,
            created_by: format!("videoindex {}", env!("CARGO_PKG_VERSION")),
            created_at: now,
            updated_at: now,
            index_id: IndexId::new(),
            files: BTreeMap::new(),
            signature: None,
        }
    }

    /// Read from an index directory.
    pub fn read(dir: &Path) -> Result<Self> {
        let p = dir.join(MANIFEST_FILE);
        if !p.is_file() {
            return Err(IndexError::NotAnIndex(dir.display().to_string()));
        }
        let m: Manifest = serde_json::from_slice(&std::fs::read(p)?)?;
        if m.schema_version > SCHEMA_VERSION {
            return Err(IndexError::SchemaTooNew {
                found: m.schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        Ok(m)
    }

    /// Write atomically.
    pub fn write(&self, dir: &Path) -> Result<()> {
        let p = dir.join(MANIFEST_FILE);
        let tmp = dir.join(format!("{MANIFEST_FILE}.tmp"));
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, p)?;
        Ok(())
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new()
    }
}
