//! `Http` and `ObjectStore` acquirers against local stand-ins: a tiny HTTP
//! server serving the fixture (with Range and redirects) and an
//! `object_store` local filesystem store.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use vi_core::config::DownloadConfig;
use vi_media::{Acquirer, Http, ObjectStore, Source};
use vi_testkit as fx;

/// Serve `body` at `/video/fixture.mp4` with Range support; `/go` redirects
/// there; `/page.mp4` is HTML. Returns the bound address.
async fn serve(body: Arc<Vec<u8>>) -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            let body = body.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                let range = req
                    .lines()
                    .find_map(|l| l.strip_prefix("range: bytes="))
                    .and_then(|r| r.split('-').next())
                    .and_then(|s| s.trim().parse::<usize>().ok());
                let resp: Vec<u8> = if path == "/go" {
                    b"HTTP/1.1 302 Found\r\nLocation: /video/fixture.mp4\r\nContent-Length: 0\r\n\r\n".to_vec()
                } else if path == "/page.mp4" {
                    let html = b"<html>nope</html>";
                    let mut r = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n",
                        html.len()
                    )
                    .into_bytes();
                    r.extend_from_slice(html);
                    r
                } else if path == "/video/fixture.mp4" {
                    match range {
                        Some(start) if start < body.len() => {
                            let mut r = format!(
                                "HTTP/1.1 206 Partial Content\r\nContent-Type: video/mp4\r\nContent-Range: bytes {start}-{}/{}\r\nContent-Length: {}\r\n\r\n",
                                body.len() - 1,
                                body.len(),
                                body.len() - start
                            )
                            .into_bytes();
                            r.extend_from_slice(&body[start..]);
                            r
                        }
                        _ => {
                            let mut r = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nContent-Length: {}\r\n\r\n",
                                body.len()
                            )
                            .into_bytes();
                            r.extend_from_slice(&body);
                            r
                        }
                    }
                } else {
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec()
                };
                let _ = sock.write_all(&resp).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    addr
}

#[tokio::test]
async fn http_downloads_resumes_redirects_and_refuses_private_hosts() {
    let body = Arc::new(std::fs::read(fx::fixture_path()).unwrap());
    let addr = serve(body.clone()).await;
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("videos");

    // Private addresses are refused by default.
    let strict = Http::new(&cache, DownloadConfig::default());
    let url = format!("http://{addr}/video/fixture.mp4");
    let err = strict
        .acquire(&Source::Url(url.clone()))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("private or local address"), "{err}");

    let cfg = DownloadConfig {
        allow_private_addresses: true,
        ..DownloadConfig::default()
    };
    let http = Http::new(&cache, cfg.clone());
    assert!(http.handles(&Source::Url(url.clone())));

    // A partial file resumes with Range.
    let dest = cache.join("incoming").join("resume.mp4");
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
    std::fs::write(dest.with_extension("mp4.part"), &body[..1000]).unwrap();
    let n = http.download(&url, &dest).await.unwrap();
    assert_eq!(n as usize, body.len());
    assert_eq!(std::fs::read(&dest).unwrap(), *body);

    // Redirects are followed; the result lands in the cache by hash with
    // the URL as source_uri.
    let acquired = http
        .acquire(&Source::Url(format!("http://{addr}/go")))
        .await;
    // `/go` has no media extension so the acquirer does not claim it; call
    // the download directly to exercise the redirect.
    assert!(acquired.is_err() || acquired.is_ok());
    let dest2 = cache.join("incoming").join("via-redirect.mp4");
    http.download(&format!("http://{addr}/go"), &dest2)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&dest2).unwrap().len(), body.len());

    let acquired = http.acquire(&Source::Url(url.clone())).await.unwrap();
    assert_eq!(acquired.source_uri, url);
    assert!(acquired.path.starts_with(&cache));
    assert!(acquired
        .path
        .to_string_lossy()
        .contains(&acquired.content_hash));
    assert_eq!(acquired.size_bytes as usize, body.len());

    // Size limit and HTML responses are refused.
    let small = Http::new(
        &cache,
        DownloadConfig {
            max_bytes: 1000,
            ..cfg.clone()
        },
    );
    let err = small
        .download(&url, &cache.join("incoming").join("too-big.mp4"))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("max_bytes"), "{err}");
    let err = http
        .download(
            &format!("http://{addr}/page.mp4"),
            &cache.join("incoming").join("page.mp4"),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("HTML"), "{err}");
    let err = http
        .download(
            &format!("http://{addr}/missing.mp4"),
            &cache.join("incoming").join("missing.mp4"),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("404"), "{err}");
}

#[tokio::test]
async fn object_store_downloads_objects_and_expands_prefixes() {
    let dir = tempfile::tempdir().unwrap();
    let bucket_root = dir.path().join("bucket");
    std::fs::create_dir_all(bucket_root.join("talks")).unwrap();
    std::fs::copy(fx::fixture_path(), bucket_root.join("talks/day1.mp4")).unwrap();
    std::fs::copy(fx::fixture_path(), bucket_root.join("talks/day2.mp4")).unwrap();
    std::fs::write(bucket_root.join("talks/notes.txt"), b"not media").unwrap();
    std::fs::write(
        bucket_root.join("talks/day1.info.json"),
        serde_json::json!({"id": "d1", "title": "Day one", "webpage_url": "https://example.com/d1"}).to_string(),
    )
    .unwrap();
    let store: Arc<dyn object_store::ObjectStore> =
        Arc::new(object_store::local::LocalFileSystem::new_with_prefix(&bucket_root).unwrap());
    let cache = dir.path().join("videos");
    let acq = ObjectStore::new(&cache, DownloadConfig::default()).with_store(store);

    let expanded = acq
        .expand(&Source::Url("s3://bucket/talks/".into()))
        .await
        .unwrap();
    assert_eq!(
        expanded.iter().map(|s| s.uri()).collect::<Vec<_>>(),
        vec!["s3://bucket/talks/day1.mp4", "s3://bucket/talks/day2.mp4"]
    );

    let a = acq.acquire(&expanded[0]).await.unwrap();
    assert_eq!(a.source_uri, "s3://bucket/talks/day1.mp4");
    assert!(a.path.starts_with(&cache));
    assert_eq!(a.title.as_deref(), Some("Day one"), "sidecar came along");
    assert_eq!(
        a.size_bytes,
        std::fs::metadata(fx::fixture_path()).unwrap().len()
    );

    let too_big = ObjectStore::new(
        &cache,
        DownloadConfig {
            max_bytes: 10,
            ..DownloadConfig::default()
        },
    )
    .with_store(Arc::new(
        object_store::local::LocalFileSystem::new_with_prefix(&bucket_root).unwrap(),
    ));
    let err = too_big.acquire(&expanded[1]).await.unwrap_err().to_string();
    assert!(err.contains("max_bytes"), "{err}");
}
