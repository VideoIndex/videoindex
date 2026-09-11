//! Acquirers resolve a Source to a local, seekable media file plus metadata.
//! M0 ships `LocalFile`; `YtDlp`, `Http`, and `ObjectStore` follow in M1.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{MediaError, Result};

/// What the user asked to index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// A path on this machine.
    Path(PathBuf),
    /// A URL (http, s3, youtube, ...).
    Url(String),
}

impl Source {
    /// Parse a CLI argument: anything with a scheme is a URL.
    pub fn parse(s: &str) -> Self {
        if s.contains("://") && !Path::new(s).exists() {
            Self::Url(s.to_string())
        } else {
            Self::Path(PathBuf::from(s))
        }
    }

    /// Display form used as `source_uri`.
    pub fn uri(&self) -> String {
        match self {
            Self::Path(p) => match p.canonicalize() {
                Ok(c) => format!("file://{}", c.display()),
                Err(_) => format!("file://{}", p.display()),
            },
            Self::Url(u) => u.clone(),
        }
    }
}

/// Result of acquisition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Acquired {
    /// Local, seekable media file.
    pub path: PathBuf,
    /// `source_uri` for the Video record.
    pub source_uri: String,
    /// blake3 of the file contents, hex.
    pub content_hash: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Title from sidecar metadata when known (M1: `.info.json`).
    pub title: Option<String>,
    /// Sidecar subtitle files found next to the media.
    pub subtitle_files: Vec<PathBuf>,
}

/// Resolves a Source to a local file.
#[async_trait]
pub trait Acquirer: Send + Sync {
    /// Whether this acquirer handles the source.
    fn handles(&self, source: &Source) -> bool;
    /// Acquire.
    async fn acquire(&self, source: &Source) -> Result<Acquired>;
}

/// Zero-copy open of a local path. Hashes the file with blake3 on a blocking
/// thread. Sidecar import (`.info.json`, `.srt`, `.vtt`) is M1 work; only the
/// file list is collected here.
#[derive(Debug, Default, Clone)]
pub struct LocalFile;

#[async_trait]
impl Acquirer for LocalFile {
    fn handles(&self, source: &Source) -> bool {
        matches!(source, Source::Path(_))
    }

    async fn acquire(&self, source: &Source) -> Result<Acquired> {
        let Source::Path(path) = source else {
            return Err(MediaError::Acquire(format!(
                "LocalFile cannot handle {source:?}"
            )));
        };
        if !path.is_file() {
            return Err(MediaError::Acquire(format!(
                "not a file: {}",
                path.display()
            )));
        }
        let path = path.canonicalize()?;
        let hash_path = path.clone();
        let (content_hash, size_bytes) = tokio::task::spawn_blocking(move || hash_file(&hash_path))
            .await
            .map_err(|e| MediaError::Acquire(format!("hash task failed: {e}")))??;
        Ok(Acquired {
            source_uri: source.uri(),
            subtitle_files: sidecar_subtitles(&path),
            path,
            content_hash,
            size_bytes,
            title: None,
        })
    }
}

/// blake3 hex digest and size of a file.
pub fn hash_file(path: &Path) -> Result<(String, u64)> {
    let mut hasher = blake3::Hasher::new();
    let mut f = std::fs::File::open(path)?;
    let size = std::io::copy(&mut f, &mut hasher)?;
    Ok((hasher.finalize().to_hex().to_string(), size))
}

/// Subtitle sidecars next to `path` with the same stem (`name.en.srt`,
/// `name.vtt`, ...).
pub fn sidecar_subtitles(path: &Path) -> Vec<PathBuf> {
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return Vec::new();
    };
    let Some(dir) = path.parent() else {
        return Vec::new();
    };
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            name.starts_with(stem) && matches!(ext, "srt" | "vtt")
        })
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_parse() {
        assert!(matches!(Source::parse("https://x/y.mp4"), Source::Url(_)));
        assert!(matches!(Source::parse("/tmp/x.mp4"), Source::Path(_)));
        assert!(matches!(Source::parse("rel/x.mp4"), Source::Path(_)));
    }

    #[tokio::test]
    async fn local_file_hashes_and_finds_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("talk.mp4");
        std::fs::write(&media, b"not really a video").unwrap();
        std::fs::write(dir.path().join("talk.en.srt"), b"1\n").unwrap();
        std::fs::write(dir.path().join("other.srt"), b"1\n").unwrap();
        let a = LocalFile
            .acquire(&Source::Path(media.clone()))
            .await
            .unwrap();
        assert_eq!(a.size_bytes, 18);
        assert_eq!(a.content_hash.len(), 64);
        assert_eq!(a.subtitle_files.len(), 1);
        assert!(a.source_uri.starts_with("file://"));
        assert!(LocalFile
            .acquire(&Source::Path(dir.path().join("missing.mp4")))
            .await
            .is_err());
    }
}
