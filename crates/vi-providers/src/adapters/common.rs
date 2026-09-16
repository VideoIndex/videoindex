//! Shared helpers for the chat adapters: image encoding, tool-call
//! accumulation, and turning a finished exchange into [`CallStats`].

use std::time::Instant;

use base64::Engine;
use vi_core::config::Pricing;

use crate::cost::{CallStats, Usage};
use crate::error::{ProviderError, Result};
use crate::traits::{GenerateEvent, ImageData};

/// Encode an image as `(mime, base64)`. Raw RGB becomes JPEG (quality 85).
pub fn image_base64(image: &ImageData) -> Result<(&'static str, String)> {
    match image {
        ImageData::Encoded { mime, bytes } => Ok((
            mime,
            base64::engine::general_purpose::STANDARD.encode(bytes),
        )),
        ImageData::Rgb8 {
            width,
            height,
            data,
        } => {
            let n = (*width as usize) * (*height as usize) * 3;
            if data.len() < n {
                return Err(ProviderError::Invalid("RGB buffer too short".into()));
            }
            let img = image::RgbImage::from_raw(*width, *height, data[..n].to_vec())
                .ok_or_else(|| ProviderError::Invalid("bad image dimensions".into()))?;
            let mut out = std::io::Cursor::new(Vec::new());
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85)
                .encode_image(&img)
                .map_err(|e| ProviderError::Invalid(format!("jpeg encode: {e}")))?;
            Ok((
                "image/jpeg",
                base64::engine::general_purpose::STANDARD.encode(out.into_inner()),
            ))
        }
    }
}

/// Accumulates streamed tool-call fragments (id, name, argument pieces).
#[derive(Debug, Default, Clone)]
pub struct ToolCallBuilder {
    /// Call id.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// JSON arguments so far.
    pub arguments: String,
}

impl ToolCallBuilder {
    /// The finished event.
    pub fn finish(self) -> GenerateEvent {
        GenerateEvent::ToolCall {
            id: self.id,
            name: self.name,
            arguments: if self.arguments.is_empty() {
                "{}".to_string()
            } else {
                self.arguments
            },
            signature: None,
        }
    }
}

/// Final accounting for a chat call.
pub fn final_stats(
    provider: &str,
    model: &str,
    usage: Usage,
    pricing: &Pricing,
    started: Instant,
    attempts: u32,
) -> CallStats {
    let mut s = CallStats::new(provider, model, usage, pricing);
    s.latency_ms = started.elapsed().as_millis() as u64;
    s.attempts = attempts;
    s
}

/// Build a `reqwest` client with the governor's timeout.
pub fn http_client(name: &str, timeout: std::time::Duration) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(std::time::Duration::from_secs(20))
        .user_agent(concat!("videoindex/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| ProviderError::Transport {
            provider: name.to_string(),
            message: e.to_string(),
        })
}

/// Turn a non-success response into an error with `Retry-After`.
pub async fn check_status(provider: &str, resp: reqwest::Response) -> Result<reqwest::Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let retry_after = resp
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(std::time::Duration::from_secs);
    let body = resp.text().await.unwrap_or_default();
    Err(ProviderError::Http {
        provider: provider.to_string(),
        status: status.as_u16(),
        body: crate::error::short_body(&body),
        retry_after,
    })
}

/// Map a transport error.
pub fn transport(provider: &str, timeout_secs: u64, e: reqwest::Error) -> ProviderError {
    if e.is_timeout() {
        ProviderError::Timeout {
            provider: provider.to_string(),
            secs: timeout_secs,
        }
    } else {
        ProviderError::Transport {
            provider: provider.to_string(),
            message: crate::error::redact(&e.without_url().to_string()),
        }
    }
}

/// Instruction appended when a schema is requested but the provider has no
/// native JSON-schema mode.
pub fn schema_instruction(schema: &serde_json::Value) -> String {
    format!(
        "Respond with a single JSON object matching this JSON Schema and nothing else:\n{}",
        schema
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn rgb_becomes_jpeg() {
        let (mime, b64) = image_base64(&ImageData::Rgb8 {
            width: 8,
            height: 4,
            data: Arc::from(vec![200u8; 8 * 4 * 3]),
        })
        .unwrap();
        assert_eq!(mime, "image/jpeg");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
    }
}
