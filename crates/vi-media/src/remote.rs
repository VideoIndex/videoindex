//! Remote acquirers: [`Http`] for `https://…/file.mp4` and [`ObjectStore`]
//! for `s3://`, `gs://`, `az://` and `r2://`. Both download into
//! `<cache_dir>/incoming/<name>` with size and time limits and then hand the
//! file to [`LocalFile`], which hashes it, imports any sidecars and moves it
//! into the content-addressed cache, so every acquirer produces the same
//! index contents.
//!
//! `Http` refuses hosts that resolve to private, loopback or link-local
//! addresses unless `media.download.allow_private_addresses` is set, and
//! re-checks every redirect target, so a hosted server cannot be used to
//! read from its own network. Downloads resume with `Range` when a partial
//! file exists.

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use object_store::ObjectStore as ObjectStoreApi;
use object_store::ObjectStoreExt as _;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};
use vi_core::config::DownloadConfig;

use crate::acquire::{Acquired, Acquirer, LocalFile, Source};
use crate::error::{MediaError, Result};

/// Extensions the HTTP acquirer treats as media.
pub const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "webm", "mov", "m4v", "avi", "ts", "mp3", "m4a", "wav", "flac",
];

fn media_extension(path: &str) -> Option<&'static str> {
    let lower = path.to_ascii_lowercase();
    let ext = lower.rsplit('.').next()?;
    MEDIA_EXTENSIONS.iter().copied().find(|e| *e == ext)
}

/// Whether an address is one a server must not be made to fetch from.
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                // Carrier-grade NAT 100.64.0.0/10 and cloud metadata 169.254/16
                // (link-local already) and 0.0.0.0/8.
                || (v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))
                || v4.octets()[0] == 0
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                // fc00::/7 unique local, fe80::/10 link local
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                // IPv4-mapped: apply the v4 rules.
                || v6.to_ipv4_mapped().is_some_and(|v4| is_private_ip(IpAddr::V4(v4)))
        }
    }
}

/// Resolve a host and refuse it when any address is private (unless
/// allowed). Returns the addresses so the connection can be pinned to them.
async fn check_host(
    host: &str,
    port: u16,
    allow_private: bool,
) -> Result<Vec<std::net::SocketAddr>> {
    let addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| MediaError::Acquire(format!("cannot resolve {host}: {e}")))?
        .collect();
    if addrs.is_empty() {
        return Err(MediaError::Acquire(format!("{host} has no addresses")));
    }
    if !allow_private {
        if let Some(bad) = addrs.iter().find(|a| is_private_ip(a.ip())) {
            return Err(MediaError::Acquire(format!(
                "{host} resolves to {} (private or local address); set media.download.allow_private_addresses to fetch from it",
                bad.ip()
            )));
        }
    }
    Ok(addrs)
}

/// File name for a download from a URL: the last path segment, or a hash of
/// the URL when there is none.
fn file_name_for(url: &url::Url) -> String {
    let last = url
        .path_segments()
        .and_then(|s| s.rev().find(|p| !p.is_empty()))
        .unwrap_or("");
    let decoded = percent_decode(last);
    let safe: String = decoded
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() || media_extension(&safe).is_none() {
        let h = blake3::hash(url.as_str().as_bytes()).to_hex();
        format!(
            "{}.{}",
            &h[..16],
            media_extension(url.path()).unwrap_or("mp4")
        )
    } else {
        format!(
            "{}-{}",
            &blake3::hash(url.as_str().as_bytes()).to_hex()[..8],
            safe
        )
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// `https://host/path/file.mp4` downloads.
#[derive(Debug, Clone)]
pub struct Http {
    cache_dir: PathBuf,
    cfg: DownloadConfig,
}

impl Http {
    /// Acquirer downloading into `<cache_dir>/incoming/`.
    pub fn new(cache_dir: impl Into<PathBuf>, cfg: DownloadConfig) -> Self {
        Self {
            cache_dir: cache_dir.into(),
            cfg,
        }
    }

    fn incoming(&self) -> PathBuf {
        self.cache_dir.join("incoming")
    }

    /// Download `url` to `dest`, resuming a partial file, following up to
    /// `max_redirects` redirects with the private-address check on each.
    pub async fn download(&self, url: &str, dest: &Path) -> Result<u64> {
        let mut current =
            url::Url::parse(url).map_err(|e| MediaError::Acquire(format!("bad URL {url}: {e}")))?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(self.cfg.timeout_secs.max(1)))
            .user_agent(concat!("videoindex/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| MediaError::Acquire(e.to_string()))?;
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let part = dest.with_extension(format!(
            "{}.part",
            dest.extension().and_then(|e| e.to_str()).unwrap_or("bin")
        ));
        let mut have = tokio::fs::metadata(&part)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        for hop in 0..=self.cfg.max_redirects {
            if !matches!(current.scheme(), "http" | "https") {
                return Err(MediaError::Acquire(format!(
                    "unsupported scheme in {current}"
                )));
            }
            let host = current
                .host_str()
                .ok_or_else(|| MediaError::Acquire(format!("no host in {current}")))?;
            let port = current.port_or_known_default().unwrap_or(443);
            check_host(host, port, self.cfg.allow_private_addresses).await?;
            let mut req = client.get(current.clone());
            if have > 0 {
                req = req.header(reqwest::header::RANGE, format!("bytes={have}-"));
            }
            let resp = req
                .send()
                .await
                .map_err(|e| MediaError::Acquire(format!("GET {current}: {}", e.without_url())))?;
            let status = resp.status();
            if status.is_redirection() {
                let loc = resp
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| {
                        MediaError::Acquire(format!("redirect without Location from {current}"))
                    })?;
                current = current
                    .join(loc)
                    .map_err(|e| MediaError::Acquire(format!("bad redirect {loc}: {e}")))?;
                debug!(hop, "following redirect to {current}");
                continue;
            }
            if status.as_u16() == 416 {
                // Range not satisfiable: the partial file is already complete
                // or the server changed the file; start over.
                let _ = tokio::fs::remove_file(&part).await;
                return Box::pin(self.download(current.as_str(), dest)).await;
            }
            if !status.is_success() {
                return Err(MediaError::Acquire(format!("GET {current}: HTTP {status}")));
            }
            let resuming = status.as_u16() == 206 && have > 0;
            if !resuming {
                have = 0;
            }
            let total = resp.content_length().map(|l| l + have).or_else(|| {
                resp.headers()
                    .get(reqwest::header::CONTENT_RANGE)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.rsplit('/').next())
                    .and_then(|t| t.parse::<u64>().ok())
            });
            if let Some(t) = total {
                if t > self.cfg.max_bytes {
                    return Err(MediaError::Acquire(format!(
                        "{current} is {t} bytes, over media.download.max_bytes ({})",
                        self.cfg.max_bytes
                    )));
                }
            }
            let ct = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ct.starts_with("text/html") {
                return Err(MediaError::Acquire(format!(
                    "{current} returned an HTML page, not a media file (is it a video-site URL? yt-dlp handles those)"
                )));
            }
            let mut file = if resuming {
                tokio::fs::OpenOptions::new()
                    .append(true)
                    .open(&part)
                    .await?
            } else {
                tokio::fs::File::create(&part).await?
            };
            let mut written = have;
            let mut stream = resp.bytes_stream();
            let started = std::time::Instant::now();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| {
                    MediaError::Acquire(format!("reading {current}: {}", e.without_url()))
                })?;
                written += chunk.len() as u64;
                if written > self.cfg.max_bytes {
                    let _ = tokio::fs::remove_file(&part).await;
                    return Err(MediaError::Acquire(format!(
                        "{current} exceeded media.download.max_bytes ({})",
                        self.cfg.max_bytes
                    )));
                }
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
            drop(file);
            if let Some(t) = total {
                if written != t {
                    return Err(MediaError::Acquire(format!(
                        "{current}: got {written} of {t} bytes; re-run to resume"
                    )));
                }
            }
            tokio::fs::rename(&part, dest).await?;
            info!(
                url = %current,
                bytes = written,
                secs = format!("{:.1}", started.elapsed().as_secs_f64()),
                "downloaded"
            );
            return Ok(written);
        }
        Err(MediaError::Acquire(format!(
            "too many redirects (over {}) from {url}",
            self.cfg.max_redirects
        )))
    }
}

#[async_trait]
impl Acquirer for Http {
    fn handles(&self, source: &Source) -> bool {
        match source {
            Source::Url(u) => {
                let lower = u.to_ascii_lowercase();
                (lower.starts_with("http://") || lower.starts_with("https://"))
                    && !source.is_video_site()
                    && url::Url::parse(u)
                        .ok()
                        .is_some_and(|p| media_extension(p.path()).is_some())
            }
            Source::Path(_) => false,
        }
    }

    async fn acquire(&self, source: &Source) -> Result<Acquired> {
        let Source::Url(u) = source else {
            return Err(MediaError::Acquire("Http acquirer needs a URL".into()));
        };
        let parsed =
            url::Url::parse(u).map_err(|e| MediaError::Acquire(format!("bad URL {u}: {e}")))?;
        let dest = self.incoming().join(file_name_for(&parsed));
        if !dest.is_file() {
            self.download(u, &dest).await?;
        } else {
            debug!("{} already downloaded", dest.display());
        }
        let local = LocalFile::with_cache(&self.cache_dir);
        let mut acquired = local.acquire(&Source::Path(dest)).await?;
        acquired.source_uri = u.clone();
        Ok(acquired)
    }
}

/// `s3://bucket/key`, `gs://bucket/key`, `az://container/key`,
/// `r2://bucket/key` downloads through the `object_store` crate.
/// Credentials come from the usual environment variables (`AWS_*`,
/// `GOOGLE_APPLICATION_CREDENTIALS` / `GOOGLE_SERVICE_ACCOUNT`, `AZURE_*`).
pub struct ObjectStore {
    cache_dir: PathBuf,
    cfg: DownloadConfig,
    /// Test hook: a store to use for every URL instead of building one.
    override_store: Option<Arc<dyn object_store::ObjectStore>>,
}

impl std::fmt::Debug for ObjectStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectStore")
            .field("cache_dir", &self.cache_dir)
            .finish()
    }
}

impl ObjectStore {
    /// Acquirer downloading into `<cache_dir>/incoming/`.
    pub fn new(cache_dir: impl Into<PathBuf>, cfg: DownloadConfig) -> Self {
        Self {
            cache_dir: cache_dir.into(),
            cfg,
            override_store: None,
        }
    }

    /// Use one store for every URL (tests, or a pre-configured client).
    pub fn with_store(mut self, store: Arc<dyn object_store::ObjectStore>) -> Self {
        self.override_store = Some(store);
        self
    }

    /// Schemes handled.
    pub const SCHEMES: &'static [&'static str] = &["s3", "gs", "az", "r2"];

    fn store_for(
        &self,
        url: &url::Url,
    ) -> Result<(Arc<dyn object_store::ObjectStore>, object_store::path::Path)> {
        let key = object_store::path::Path::from(url.path().trim_start_matches('/'));
        if let Some(s) = &self.override_store {
            return Ok((s.clone(), key));
        }
        let bucket = url
            .host_str()
            .ok_or_else(|| MediaError::Acquire(format!("no bucket in {url}")))?;
        let store: Arc<dyn object_store::ObjectStore> = match url.scheme() {
            "s3" => Arc::new(
                object_store::aws::AmazonS3Builder::from_env()
                    .with_bucket_name(bucket)
                    .build()
                    .map_err(|e| MediaError::Acquire(format!("s3: {e}")))?,
            ),
            "r2" => {
                let endpoint = self
                    .cfg
                    .r2_endpoint
                    .clone()
                    .or_else(|| std::env::var("R2_ENDPOINT").ok())
                    .ok_or_else(|| {
                        MediaError::Acquire(
                            "r2:// needs media.download.r2_endpoint or R2_ENDPOINT (https://<account>.r2.cloudflarestorage.com)".into(),
                        )
                    })?;
                Arc::new(
                    object_store::aws::AmazonS3Builder::from_env()
                        .with_bucket_name(bucket)
                        .with_endpoint(endpoint)
                        .with_region("auto")
                        .build()
                        .map_err(|e| MediaError::Acquire(format!("r2: {e}")))?,
                )
            }
            "gs" => Arc::new(
                object_store::gcp::GoogleCloudStorageBuilder::from_env()
                    .with_bucket_name(bucket)
                    .build()
                    .map_err(|e| MediaError::Acquire(format!("gcs: {e}")))?,
            ),
            "az" => Arc::new(
                object_store::azure::MicrosoftAzureBuilder::from_env()
                    .with_container_name(bucket)
                    .build()
                    .map_err(|e| MediaError::Acquire(format!("azure: {e}")))?,
            ),
            other => {
                return Err(MediaError::Acquire(format!(
                    "unsupported scheme {other}://"
                )))
            }
        };
        Ok((store, key))
    }

    /// Download one object to `dest`.
    pub async fn download(&self, url: &str, dest: &Path) -> Result<u64> {
        let parsed =
            url::Url::parse(url).map_err(|e| MediaError::Acquire(format!("bad URL {url}: {e}")))?;
        let (store, key) = self.store_for(&parsed)?;
        let meta = store
            .head(&key)
            .await
            .map_err(|e| MediaError::Acquire(format!("{url}: {e}")))?;
        if meta.size > self.cfg.max_bytes {
            return Err(MediaError::Acquire(format!(
                "{url} is {} bytes, over media.download.max_bytes ({})",
                meta.size, self.cfg.max_bytes
            )));
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let part = dest.with_extension("part");
        let mut file = tokio::fs::File::create(&part).await?;
        let started = std::time::Instant::now();
        let result = store
            .get(&key)
            .await
            .map_err(|e| MediaError::Acquire(format!("{url}: {e}")))?;
        let mut stream = result.into_stream();
        let mut written = 0u64;
        let deadline = started + Duration::from_secs(self.cfg.timeout_secs.max(1));
        while let Some(chunk) = stream.next().await {
            if std::time::Instant::now() > deadline {
                let _ = tokio::fs::remove_file(&part).await;
                return Err(MediaError::Acquire(format!(
                    "{url}: download exceeded media.download.timeout_secs"
                )));
            }
            let chunk = chunk.map_err(|e| MediaError::Acquire(format!("{url}: {e}")))?;
            written += chunk.len() as u64;
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        drop(file);
        if written != meta.size {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(MediaError::Acquire(format!(
                "{url}: got {written} of {} bytes",
                meta.size
            )));
        }
        tokio::fs::rename(&part, dest).await?;
        info!(
            url,
            bytes = written,
            secs = format!("{:.1}", started.elapsed().as_secs_f64()),
            "downloaded"
        );
        Ok(written)
    }
}

#[async_trait]
impl Acquirer for ObjectStore {
    fn handles(&self, source: &Source) -> bool {
        match source {
            Source::Url(u) => Self::SCHEMES
                .iter()
                .any(|s| u.to_ascii_lowercase().starts_with(&format!("{s}://"))),
            Source::Path(_) => false,
        }
    }

    async fn expand(&self, source: &Source) -> Result<Vec<Source>> {
        // A prefix ending in '/' expands to the media objects under it.
        let Source::Url(u) = source else {
            return Ok(vec![source.clone()]);
        };
        if !u.ends_with('/') {
            return Ok(vec![source.clone()]);
        }
        let parsed =
            url::Url::parse(u).map_err(|e| MediaError::Acquire(format!("bad URL {u}: {e}")))?;
        let (store, prefix) = self.store_for(&parsed)?;
        let mut listing = store.list(Some(&prefix));
        let mut out = Vec::new();
        while let Some(item) = listing.next().await {
            let meta = item.map_err(|e| MediaError::Acquire(format!("{u}: {e}")))?;
            let loc = meta.location.to_string();
            if media_extension(&loc).is_some() {
                out.push(Source::Url(format!(
                    "{}://{}/{}",
                    parsed.scheme(),
                    parsed.host_str().unwrap_or(""),
                    loc
                )));
            }
        }
        out.sort_by_key(|a| a.uri());
        Ok(out)
    }

    async fn acquire(&self, source: &Source) -> Result<Acquired> {
        let Source::Url(u) = source else {
            return Err(MediaError::Acquire(
                "ObjectStore acquirer needs a URL".into(),
            ));
        };
        let parsed =
            url::Url::parse(u).map_err(|e| MediaError::Acquire(format!("bad URL {u}: {e}")))?;
        let dest = self.incoming().join(file_name_for(&parsed));
        if !dest.is_file() {
            self.download(u, &dest).await?;
            // Sidecars next to the object (`.info.json`, `.srt`) come along
            // when present.
            let (store, key) = self.store_for(&parsed)?;
            let stem = key.to_string();
            let stem = stem
                .rsplit_once('.')
                .map(|(s, _)| s.to_string())
                .unwrap_or(stem);
            for suffix in [".info.json", ".en.srt", ".srt", ".en.vtt", ".vtt"] {
                let side = object_store::path::Path::from(format!("{stem}{suffix}"));
                if let Ok(r) = store.get(&side).await {
                    if let Ok(bytes) = r.bytes().await {
                        let local_name = format!(
                            "{}{suffix}",
                            dest.file_stem().and_then(|s| s.to_str()).unwrap_or("media")
                        );
                        let _ = tokio::fs::write(dest.with_file_name(local_name), &bytes).await;
                    }
                }
            }
        }
        let local = LocalFile::with_cache(&self.cache_dir);
        let mut acquired = local.acquire(&Source::Path(dest)).await?;
        acquired.source_uri = u.clone();
        Ok(acquired)
    }

    fn incoming_dir(&self) -> Option<PathBuf> {
        Some(self.incoming())
    }
}

impl ObjectStore {
    fn incoming(&self) -> PathBuf {
        self.cache_dir.join("incoming")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_ranges() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "192.168.1.1",
            "172.16.0.9",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "::1",
            "fe80::1",
            "fd00::5",
        ] {
            assert!(is_private_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "104.21.65.27", "2606:4700:3033::ac43:8bdf"] {
            assert!(!is_private_ip(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn file_names_are_stable_and_safe() {
        let u = url::Url::parse("https://cdn.example.com/talks/My%20Talk.mp4?sig=abc").unwrap();
        let n = file_name_for(&u);
        assert!(n.ends_with("-My_Talk.mp4"), "{n}");
        assert_eq!(n, file_name_for(&u));
        let u2 = url::Url::parse("https://cdn.example.com/stream").unwrap();
        assert!(file_name_for(&u2).ends_with(".mp4"));
    }

    #[test]
    fn handles_only_media_urls() {
        let h = Http::new("/tmp/x", DownloadConfig::default());
        assert!(h.handles(&Source::Url("https://cdn.example.com/a/b.mp4".into())));
        assert!(h.handles(&Source::Url("http://cdn.example.com/a/b.MKV?x=1".into())));
        assert!(!h.handles(&Source::Url("https://www.youtube.com/watch?v=abc".into())));
        assert!(!h.handles(&Source::Url("https://example.com/page.html".into())));
        assert!(!h.handles(&Source::Url("s3://bucket/key.mp4".into())));
        let o = ObjectStore::new("/tmp/x", DownloadConfig::default());
        assert!(o.handles(&Source::Url("s3://bucket/key.mp4".into())));
        assert!(o.handles(&Source::Url("gs://bucket/dir/".into())));
        assert!(!o.handles(&Source::Url("https://x/y.mp4".into())));
    }
}
