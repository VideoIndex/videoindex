//! Acquirers resolve a Source to a local, seekable media file plus metadata.
//!
//! - [`LocalFile`]: opens a path, hashes it, imports yt-dlp `.info.json` and
//!   subtitle sidecars, and moves files that sit in the media cache's
//!   `incoming/` directory into the content-addressed cache.
//! - [`YtDlp`]: shells out to `yt-dlp` with a fixed argument list into
//!   `incoming/`, then hands over to `LocalFile`. Playlists expand into one
//!   Source per entry.
//!
//! `Http` and `ObjectStore` follow later in M1.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::{MediaError, Result};
use crate::sidecar::{self, InfoJson, Sidecars, SubtitleFile};

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

    /// True for YouTube and other video-site URLs yt-dlp handles.
    pub fn is_video_site(&self) -> bool {
        match self {
            Self::Url(u) => {
                let u = u.to_ascii_lowercase();
                !u.ends_with(".mp4")
                    && !u.ends_with(".mkv")
                    && !u.ends_with(".webm")
                    && !u.ends_with(".mov")
                    && (u.contains("youtube.com/")
                        || u.contains("youtu.be/")
                        || u.contains("vimeo.com/")
                        || u.contains("twitch.tv/"))
            }
            Self::Path(_) => false,
        }
    }
}

/// Result of acquisition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Acquired {
    /// Local, seekable media file (after any move into the cache).
    pub path: PathBuf,
    /// `source_uri` for the Video record: the page URL when a sidecar knows
    /// it, else the file URI.
    pub source_uri: String,
    /// blake3 of the file contents, hex.
    pub content_hash: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Title from sidecar metadata when known.
    pub title: Option<String>,
    /// Description from sidecar metadata.
    pub description: Option<String>,
    /// Channel or uploader from sidecar metadata.
    pub channel: Option<String>,
    /// Publication time from sidecar metadata.
    pub published_at: Option<DateTime<Utc>>,
    /// Chapters from sidecar metadata.
    pub chapters: Vec<sidecar::SidecarChapter>,
    /// Subtitle sidecars found next to the media (after any move).
    pub subtitle_files: Vec<SubtitleFile>,
    /// Languages with human-authored subtitles per the sidecar.
    pub subtitle_languages: Vec<String>,
    /// The parsed `.info.json`, if any.
    pub info: Option<InfoJson>,
}

impl Acquired {
    /// Whether a subtitle file is human-authored according to `.info.json`
    /// (`subtitles`) rather than an automatic caption.
    pub fn subtitle_is_human(&self, file: &SubtitleFile) -> bool {
        let Some(lang) = &file.language else {
            return self.info.is_none();
        };
        let base = lang.split('-').next().unwrap_or(lang).to_ascii_lowercase();
        self.subtitle_languages
            .iter()
            .any(|l| l.eq_ignore_ascii_case(lang) || l.eq_ignore_ascii_case(&base))
    }
}

/// Resolves a Source to a local file.
#[async_trait]
pub trait Acquirer: Send + Sync {
    /// Whether this acquirer handles the source.
    fn handles(&self, source: &Source) -> bool;
    /// Expand playlists and directories into individual sources. The default
    /// returns the source itself.
    async fn expand(&self, source: &Source) -> Result<Vec<Source>> {
        Ok(vec![source.clone()])
    }
    /// Acquire one source.
    async fn acquire(&self, source: &Source) -> Result<Acquired>;
    /// Where the acquirer downloads to, if it downloads.
    fn incoming_dir(&self) -> Option<PathBuf> {
        None
    }
}

/// Local files. Zero-copy open; sidecars imported; files under
/// `<cache_dir>/incoming/` are moved into `<cache_dir>/<hash>.<ext>` together
/// with their sidecars.
#[derive(Debug, Default, Clone)]
pub struct LocalFile {
    /// Content-addressed media cache; `None` disables moving.
    pub cache_dir: Option<PathBuf>,
}

impl LocalFile {
    /// Acquirer that leaves files where they are.
    pub fn new() -> Self {
        Self::default()
    }

    /// Acquirer that moves files from `<cache_dir>/incoming/` into the cache.
    pub fn with_cache(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: Some(cache_dir.into()),
        }
    }

    /// Where the incoming directory lives, if a cache is configured.
    pub fn incoming_dir(&self) -> Option<PathBuf> {
        self.cache_dir.as_ref().map(|c| c.join("incoming"))
    }

    fn should_move(&self, path: &Path) -> bool {
        match self.incoming_dir() {
            Some(inc) => match (inc.canonicalize(), path.canonicalize()) {
                (Ok(inc), Ok(p)) => p.starts_with(inc),
                _ => false,
            },
            None => false,
        }
    }

    /// Move media and sidecars into the cache under the content hash.
    fn move_into_cache(
        &self,
        path: &Path,
        hash: &str,
        sidecars: &Sidecars,
    ) -> Result<(PathBuf, Sidecars)> {
        let cache = self
            .cache_dir
            .as_ref()
            .ok_or_else(|| MediaError::Acquire("no cache dir".into()))?;
        std::fs::create_dir_all(cache)?;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mp4")
            .to_ascii_lowercase();
        let dest = cache.join(format!("{hash}.{ext}"));
        if dest.exists() {
            // Same content already cached: drop the duplicate incoming copy.
            std::fs::remove_file(path)?;
            info!(
                "{} already in cache as {}; removed duplicate",
                path.display(),
                dest.display()
            );
        } else {
            rename_or_copy(path, &dest)?;
        }
        let mut moved = Sidecars::default();
        if let Some(info) = &sidecars.info {
            let d = cache.join(format!("{hash}.info.json"));
            if !d.exists() {
                rename_or_copy(&info.path, &d)?;
            } else {
                let _ = std::fs::remove_file(&info.path);
            }
            moved.info = InfoJson::read(&d).ok();
        }
        for s in &sidecars.subtitles {
            let ext = match s.format {
                sidecar::SubtitleFormat::Srt => "srt",
                sidecar::SubtitleFormat::Vtt => "vtt",
            };
            let name = match &s.language {
                Some(l) => format!("{hash}.{l}.{ext}"),
                None => format!("{hash}.{ext}"),
            };
            let d = cache.join(name);
            if !d.exists() {
                rename_or_copy(&s.path, &d)?;
            } else {
                let _ = std::fs::remove_file(&s.path);
            }
            moved.subtitles.push(SubtitleFile {
                path: d,
                language: s.language.clone(),
                format: s.format,
            });
        }
        Ok((dest, moved))
    }
}

fn rename_or_copy(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            std::fs::copy(from, to)?;
            std::fs::remove_file(from)?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

#[async_trait]
impl Acquirer for LocalFile {
    fn handles(&self, source: &Source) -> bool {
        matches!(source, Source::Path(_))
    }

    async fn expand(&self, source: &Source) -> Result<Vec<Source>> {
        // A directory expands to its video files, sorted.
        let Source::Path(p) = source else {
            return Ok(vec![source.clone()]);
        };
        if !p.is_dir() {
            return Ok(vec![source.clone()]);
        }
        let mut out: Vec<Source> = std::fs::read_dir(p)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|f| is_video_file(f))
            .map(Source::Path)
            .collect();
        out.sort_by_key(|s| match s {
            Source::Path(p) => p.clone(),
            Source::Url(u) => PathBuf::from(u),
        });
        Ok(out)
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
        let mut sidecars = sidecar::find(&path);
        let mut final_path = path.clone();
        if self.should_move(&path) {
            let (p, sc) = self.move_into_cache(&path, &content_hash, &sidecars)?;
            debug!("moved {} -> {}", path.display(), p.display());
            final_path = p;
            sidecars = sc;
        }
        let info = sidecars.info.clone();
        let source_uri = info
            .as_ref()
            .and_then(|i| i.webpage_url.clone())
            .unwrap_or_else(|| Source::Path(final_path.clone()).uri());
        Ok(Acquired {
            path: final_path,
            source_uri,
            content_hash,
            size_bytes,
            title: info.as_ref().and_then(|i| i.title.clone()),
            description: info.as_ref().and_then(|i| i.description.clone()),
            channel: info.as_ref().and_then(|i| i.channel.clone()),
            published_at: info.as_ref().and_then(|i| i.upload_date),
            chapters: info
                .as_ref()
                .map(|i| i.chapters.clone())
                .unwrap_or_default(),
            subtitle_files: sidecars.subtitles,
            subtitle_languages: info
                .as_ref()
                .map(|i| i.subtitle_languages.clone())
                .unwrap_or_default(),
            info,
        })
    }
}

/// Whether a path looks like a video container.
pub fn is_video_file(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("mp4" | "mkv" | "webm" | "mov" | "m4v" | "avi" | "ts" | "mpg" | "mpeg")
    )
}

/// blake3 hex digest and size of a file.
pub fn hash_file(path: &Path) -> Result<(String, u64)> {
    let mut hasher = blake3::Hasher::new();
    let mut f = std::fs::File::open(path)?;
    let size = std::io::copy(&mut f, &mut hasher)?;
    Ok((hasher.finalize().to_hex().to_string(), size))
}

/// Subtitle sidecars next to `path` (kept for callers that only want paths).
pub fn sidecar_subtitles(path: &Path) -> Vec<PathBuf> {
    sidecar::find(path)
        .subtitles
        .into_iter()
        .map(|s| s.path)
        .collect()
}

// ------------------------------------------------------------------ yt-dlp

/// Downloads with `yt-dlp` into `<cache_dir>/incoming/`, then acquires the
/// result through [`LocalFile`]. The argument list is fixed; the URL is the
/// only user-controlled value and is passed after `--` so it can never be
/// read as a flag.
#[derive(Debug, Clone)]
pub struct YtDlp {
    /// Media cache root.
    pub cache_dir: PathBuf,
    /// `yt-dlp` executable.
    pub executable: PathBuf,
    /// Max video height.
    pub max_height: u32,
    /// Subtitle languages to request (`en.*,en`).
    pub sub_langs: String,
    /// Wall-clock limit per download.
    pub timeout: std::time::Duration,
}

impl YtDlp {
    /// Defaults matching `scripts/download_videos.sh`.
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
            executable: PathBuf::from("yt-dlp"),
            max_height: 720,
            sub_langs: "en.*,en".to_string(),
            timeout: std::time::Duration::from_secs(4 * 3600),
        }
    }

    /// Whether `yt-dlp` can be found.
    pub fn available(&self) -> bool {
        which_exists(&self.executable)
    }

    fn missing_error(&self) -> MediaError {
        MediaError::Acquire(format!(
            "yt-dlp not found ({}). Install it: `pipx install yt-dlp`, `brew install yt-dlp`, or download the binary from https://github.com/yt-dlp/yt-dlp/releases. Note that YouTube blocks most datacenter IPs; on servers, download elsewhere and transfer into {}",
            self.executable.display(),
            self.cache_dir.join("incoming").display()
        ))
    }

    fn incoming(&self) -> PathBuf {
        self.cache_dir.join("incoming")
    }
}

fn which_exists(exe: &Path) -> bool {
    if exe.components().count() > 1 {
        return exe.is_file();
    }
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|d| d.join(exe).is_file()))
        .unwrap_or(false)
}

#[async_trait]
impl Acquirer for YtDlp {
    fn handles(&self, source: &Source) -> bool {
        source.is_video_site()
    }

    async fn expand(&self, source: &Source) -> Result<Vec<Source>> {
        let Source::Url(url) = source else {
            return Ok(vec![source.clone()]);
        };
        if !self.available() {
            return Err(self.missing_error());
        }
        // Single videos with a `list=` parameter are treated as the playlist,
        // matching the download script.
        let out = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            tokio::process::Command::new(&self.executable)
                .args([
                    "--flat-playlist",
                    "--yes-playlist",
                    "--print",
                    "%(webpage_url)s",
                    "--no-warnings",
                    "--",
                ])
                .arg(url)
                .stdin(Stdio::null())
                .stderr(Stdio::piped())
                .stdout(Stdio::piped())
                .output(),
        )
        .await
        .map_err(|_| MediaError::Acquire("yt-dlp playlist expansion timed out".into()))??;
        if !out.status.success() {
            return Err(MediaError::Acquire(format!(
                "yt-dlp failed to expand {url}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        let urls: Vec<Source> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with("http"))
            .map(|l| Source::Url(l.to_string()))
            .collect();
        if urls.is_empty() {
            return Ok(vec![source.clone()]);
        }
        Ok(urls)
    }

    async fn acquire(&self, source: &Source) -> Result<Acquired> {
        let Source::Url(url) = source else {
            return Err(MediaError::Acquire(format!(
                "YtDlp cannot handle {source:?}"
            )));
        };
        if !self.available() {
            return Err(self.missing_error());
        }
        let incoming = self.incoming();
        std::fs::create_dir_all(&incoming)?;
        let format = format!(
            "bv*[height<={h}][ext=mp4]+ba[ext=m4a]/bv*[height<={h}]+ba/b[height<={h}]/b",
            h = self.max_height
        );
        let output_tpl = incoming.join("%(id)s.%(ext)s");
        let archive = incoming.join("archive.txt");
        // `--print after_move:filepath` prints the final merged file path.
        let mut cmd = tokio::process::Command::new(&self.executable);
        cmd.args([
            "--no-playlist",
            "--format",
            &format,
            "--merge-output-format",
            "mp4",
        ])
        .args(["--write-info-json", "--write-subs", "--write-auto-subs"])
        .args(["--sub-langs", &self.sub_langs, "--convert-subs", "srt"])
        .args(["--embed-chapters", "--no-overwrites", "--continue"])
        .args(["--retries", "10", "--fragment-retries", "10"])
        .arg("--download-archive")
        .arg(&archive)
        .arg("--output")
        .arg(&output_tpl)
        .args([
            "--print",
            "after_move:filepath",
            "--no-simulate",
            "--no-warnings",
            "--",
        ])
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
        info!("yt-dlp downloading {url} into {}", incoming.display());
        let out = tokio::time::timeout(self.timeout, cmd.output())
            .await
            .map_err(|_| MediaError::Acquire(format!("yt-dlp timed out downloading {url}")))??;
        let stdout = String::from_utf8_lossy(&out.stdout);
        if !out.status.success() {
            return Err(MediaError::Acquire(format!(
                "yt-dlp failed for {url}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        let path = stdout
            .lines()
            .map(str::trim)
            .rfind(|l| !l.is_empty())
            .map(PathBuf::from)
            .filter(|p| p.is_file());
        let path = match path {
            Some(p) => p,
            None => {
                // Archive hit (already downloaded): find the file by id.
                warn!("yt-dlp printed no path for {url}; searching incoming by id");
                find_by_url_id(&incoming, url).ok_or_else(|| {
                    MediaError::Acquire(format!(
                        "yt-dlp finished but no media file was found for {url}"
                    ))
                })?
            }
        };
        LocalFile::with_cache(&self.cache_dir)
            .acquire(&Source::Path(path))
            .await
    }
}

fn find_by_url_id(dir: &Path, url: &str) -> Option<PathBuf> {
    let id = url
        .split(['?', '&'])
        .find_map(|kv| kv.strip_prefix("v="))
        .or_else(|| url.rsplit('/').next())?;
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| {
            is_video_file(p)
                && p.file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s == id)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_parse() {
        assert!(matches!(Source::parse("https://x/y.mp4"), Source::Url(_)));
        assert!(matches!(Source::parse("/tmp/x.mp4"), Source::Path(_)));
        assert!(matches!(Source::parse("rel/x.mp4"), Source::Path(_)));
        assert!(Source::parse("https://www.youtube.com/watch?v=abc").is_video_site());
        assert!(!Source::parse("https://cdn.example.com/talk.mp4").is_video_site());
    }

    #[tokio::test]
    async fn local_file_hashes_and_finds_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("talk.mp4");
        std::fs::write(&media, b"not really a video").unwrap();
        std::fs::write(dir.path().join("talk.en.srt"), b"1\n").unwrap();
        std::fs::write(dir.path().join("other.srt"), b"1\n").unwrap();
        let a = LocalFile::new()
            .acquire(&Source::Path(media.clone()))
            .await
            .unwrap();
        assert_eq!(a.size_bytes, 18);
        assert_eq!(a.content_hash.len(), 64);
        assert_eq!(a.subtitle_files.len(), 1);
        assert!(a.source_uri.starts_with("file://"));
        assert!(a.info.is_none());
        assert!(LocalFile::new()
            .acquire(&Source::Path(dir.path().join("missing.mp4")))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn incoming_files_move_into_the_cache_with_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("videos");
        let incoming = cache.join("incoming").join("PLcfp");
        std::fs::create_dir_all(&incoming).unwrap();
        let media = incoming.join("001-abc.mp4");
        std::fs::write(&media, b"video bytes").unwrap();
        std::fs::write(
            incoming.join("001-abc.info.json"),
            r#"{"id":"abc","title":"T","webpage_url":"https://www.youtube.com/watch?v=abc","subtitles":{"en":[]}}"#,
        )
        .unwrap();
        std::fs::write(
            incoming.join("001-abc.en.srt"),
            "1\n00:00:00,000 --> 00:00:01,000\nhi\n",
        )
        .unwrap();

        let a = LocalFile::with_cache(&cache)
            .acquire(&Source::Path(media.clone()))
            .await
            .unwrap();
        assert!(!media.exists(), "moved out of incoming");
        assert_eq!(a.path, cache.join(format!("{}.mp4", a.content_hash)));
        assert!(cache
            .join(format!("{}.info.json", a.content_hash))
            .is_file());
        assert_eq!(a.subtitle_files.len(), 1);
        assert_eq!(
            a.subtitle_files[0].path,
            cache.join(format!("{}.en.srt", a.content_hash))
        );
        assert!(a.subtitle_is_human(&a.subtitle_files[0]));
        assert_eq!(a.source_uri, "https://www.youtube.com/watch?v=abc");
        assert_eq!(a.title.as_deref(), Some("T"));

        // A second copy of the same bytes in incoming is deduplicated.
        let dup = incoming.join("002-abc.mp4");
        std::fs::write(&dup, b"video bytes").unwrap();
        let b = LocalFile::with_cache(&cache)
            .acquire(&Source::Path(dup.clone()))
            .await
            .unwrap();
        assert_eq!(b.content_hash, a.content_hash);
        assert!(!dup.exists());

        // Files outside incoming stay put.
        let elsewhere = dir.path().join("elsewhere.mp4");
        std::fs::write(&elsewhere, b"other").unwrap();
        let c = LocalFile::with_cache(&cache)
            .acquire(&Source::Path(elsewhere.clone()))
            .await
            .unwrap();
        assert_eq!(c.path, elsewhere.canonicalize().unwrap());
    }

    #[tokio::test]
    async fn directory_expands_to_video_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.mp4"), b"").unwrap();
        std::fs::write(dir.path().join("a.mkv"), b"").unwrap();
        std::fs::write(dir.path().join("a.srt"), b"").unwrap();
        let v = LocalFile::new()
            .expand(&Source::Path(dir.path().to_path_buf()))
            .await
            .unwrap();
        assert_eq!(v.len(), 2);
        assert!(matches!(&v[0], Source::Path(p) if p.ends_with("a.mkv")));
    }

    #[tokio::test]
    async fn ytdlp_missing_is_a_clear_error() {
        let y = YtDlp {
            executable: PathBuf::from("/definitely/not/here/yt-dlp"),
            ..YtDlp::new("/tmp/vi-cache")
        };
        let err = y
            .acquire(&Source::Url("https://www.youtube.com/watch?v=abc".into()))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("yt-dlp not found"), "{err}");
        assert!(err.to_string().contains("incoming"));
    }
}
