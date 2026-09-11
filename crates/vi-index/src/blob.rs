//! Content-addressed blob store under `blobs/ab/cd/<hex>`.

use std::path::{Path, PathBuf};

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::error::{IndexError, Result};

/// A blob key: the blake3 hex digest of the content.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlobKey(pub String);

impl BlobKey {
    /// Key for some bytes.
    pub fn for_bytes(bytes: &[u8]) -> Self {
        Self(blake3::hash(bytes).to_hex().to_string())
    }

    /// Parse a stored key.
    pub fn parse(s: &str) -> Result<Self> {
        if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(IndexError::Invalid(format!("bad blob key '{s}'")));
        }
        Ok(Self(s.to_ascii_lowercase()))
    }

    /// Relative path inside `blobs/`.
    pub fn relative_path(&self) -> PathBuf {
        PathBuf::from(&self.0[0..2])
            .join(&self.0[2..4])
            .join(&self.0)
    }

    /// `blob:ab/cd/...` URI form used in query results.
    pub fn uri(&self) -> String {
        format!("blob:{}", self.relative_path().display())
    }
}

impl std::fmt::Display for BlobKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Filesystem blob store.
#[derive(Debug, Clone)]
pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    /// Store rooted at `root` (created on demand).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path(&self, key: &BlobKey) -> PathBuf {
        self.root.join(key.relative_path())
    }

    /// Write a blob under `key`. Existing content is left alone (same key,
    /// same bytes).
    pub async fn put(&self, key: &BlobKey, bytes: Bytes) -> Result<()> {
        let path = self.path(key);
        if tokio::fs::try_exists(&path).await? {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let tmp = path.with_extension(format!("tmp{}", std::process::id()));
        tokio::fs::write(&tmp, &bytes).await?;
        tokio::fs::rename(&tmp, &path).await?;
        Ok(())
    }

    /// Store bytes under their own hash and return the key.
    pub async fn put_content(&self, bytes: Bytes) -> Result<BlobKey> {
        let key = BlobKey::for_bytes(&bytes);
        self.put(&key, bytes).await?;
        Ok(key)
    }

    /// Read a blob.
    pub async fn get(&self, key: &BlobKey) -> Result<Option<Bytes>> {
        match tokio::fs::read(self.path(key)).await {
            Ok(v) => Ok(Some(Bytes::from(v))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Whether a blob exists.
    pub async fn contains(&self, key: &BlobKey) -> Result<bool> {
        Ok(tokio::fs::try_exists(self.path(key)).await?)
    }

    /// Count and total size of stored blobs (walks the tree; blocking).
    pub fn stats_blocking(&self) -> Result<(u64, u64)> {
        let mut count = 0u64;
        let mut bytes = 0u64;
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let rd = match std::fs::read_dir(&dir) {
                Ok(rd) => rd,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            for entry in rd {
                let entry = entry?;
                let ft = entry.file_type()?;
                if ft.is_dir() {
                    stack.push(entry.path());
                } else if ft.is_file() {
                    count += 1;
                    bytes += entry.metadata()?.len();
                }
            }
        }
        Ok((count, bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn put_get_roundtrip_and_layout() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new(dir.path().join("blobs"));
        let key = store
            .put_content(Bytes::from_static(b"hello"))
            .await
            .unwrap();
        assert_eq!(key.0.len(), 64);
        assert!(dir
            .path()
            .join("blobs")
            .join(&key.0[0..2])
            .join(&key.0[2..4])
            .join(&key.0)
            .is_file());
        assert_eq!(
            store.get(&key).await.unwrap().unwrap(),
            Bytes::from_static(b"hello")
        );
        assert!(store
            .get(&BlobKey::for_bytes(b"other"))
            .await
            .unwrap()
            .is_none());
        // Idempotent.
        store.put(&key, Bytes::from_static(b"hello")).await.unwrap();
        assert_eq!(store.stats_blocking().unwrap(), (1, 5));
        assert!(BlobKey::parse("zz").is_err());
        assert!(key.uri().starts_with("blob:"));
    }
}
