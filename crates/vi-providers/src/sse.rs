//! Server-sent events over a `reqwest` byte stream, and the JSON-lines
//! variant some servers use. Produces `(event, data)` pairs; `[DONE]`
//! sentinels are passed through for the adapter to interpret.

use bytes::Bytes;
use futures::stream::{Stream, StreamExt};

use crate::error::{ProviderError, Result};

/// One event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// `event:` field, empty when absent.
    pub event: String,
    /// Concatenated `data:` lines.
    pub data: String,
}

/// Parse an SSE body into events. Lines are split on `\n` (with `\r`
/// stripped); a blank line ends an event. A body without any `data:`
/// prefix is treated as JSON lines, one event per line.
pub fn events<S>(body: S, provider: String) -> futures::stream::BoxStream<'static, Result<SseEvent>>
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Unpin + Send + 'static,
{
    struct State<S> {
        body: S,
        buf: String,
        pending: std::collections::VecDeque<SseEvent>,
        done: bool,
        provider: String,
    }
    fn drain(buf: &mut String, out: &mut std::collections::VecDeque<SseEvent>, flush: bool) {
        // Split off complete events (terminated by a blank line).
        while let Some(idx) = find_event_end(buf) {
            let block: String = buf.drain(..idx.0).collect();
            buf.drain(..idx.1 - idx.0);
            if let Some(ev) = parse_block(&block) {
                out.push_back(ev);
            }
        }
        if flush && !buf.trim().is_empty() {
            let block = std::mem::take(buf);
            if let Some(ev) = parse_block(&block) {
                out.push_back(ev);
            }
        }
    }
    /// Index of the end of the first complete event and the index after
    /// its terminator.
    fn find_event_end(buf: &str) -> Option<(usize, usize)> {
        let bytes = buf.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\n' {
                // "\n\n" or "\n\r\n"
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    return Some((i, i + 2));
                }
                if i + 2 < bytes.len() && bytes[i + 1] == b'\r' && bytes[i + 2] == b'\n' {
                    return Some((i, i + 3));
                }
            }
            i += 1;
        }
        None
    }
    fn parse_block(block: &str) -> Option<SseEvent> {
        let mut event = String::new();
        let mut data: Vec<&str> = Vec::new();
        let mut saw_field = false;
        for line in block.lines() {
            let line = line.trim_end_matches('\r');
            if line.is_empty() || line.starts_with(':') {
                continue;
            }
            if let Some(v) = line.strip_prefix("data:") {
                data.push(v.strip_prefix(' ').unwrap_or(v));
                saw_field = true;
            } else if let Some(v) = line.strip_prefix("event:") {
                event = v.trim().to_string();
                saw_field = true;
            } else if line.starts_with("id:") || line.starts_with("retry:") {
                saw_field = true;
            } else if !saw_field {
                // JSON lines: the whole line is the payload.
                data.push(line);
            }
        }
        if data.is_empty() && event.is_empty() {
            return None;
        }
        Some(SseEvent {
            event,
            data: data.join("\n"),
        })
    }
    Box::pin(futures::stream::unfold(
        State {
            body,
            buf: String::new(),
            pending: std::collections::VecDeque::new(),
            done: false,
            provider,
        },
        |mut st| async move {
            loop {
                if let Some(ev) = st.pending.pop_front() {
                    return Some((Ok(ev), st));
                }
                if st.done {
                    return None;
                }
                match st.body.next().await {
                    Some(Ok(chunk)) => {
                        st.buf.push_str(&String::from_utf8_lossy(&chunk));
                        drain(&mut st.buf, &mut st.pending, false);
                    }
                    Some(Err(e)) => {
                        st.done = true;
                        return Some((
                            Err(ProviderError::Transport {
                                provider: st.provider.clone(),
                                message: e.without_url().to_string(),
                            }),
                            st,
                        ));
                    }
                    None => {
                        st.done = true;
                        drain(&mut st.buf, &mut st.pending, true);
                    }
                }
            }
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn collect(chunks: &[&str]) -> Vec<SseEvent> {
        let items: Vec<std::result::Result<Bytes, reqwest::Error>> = chunks
            .iter()
            .map(|c| Ok(Bytes::from(c.to_string())))
            .collect();
        let body = futures::stream::iter(items);
        events(body, "t".into())
            .map(|e| e.unwrap())
            .collect::<Vec<_>>()
            .await
    }

    #[tokio::test]
    async fn parses_events_across_chunk_boundaries() {
        let evs = collect(&[
            "event: message_start\ndata: {\"a\":1}\n\n",
            "data: {\"b\":",
            "2}\n\ndata: [DONE]\n\n",
        ])
        .await;
        assert_eq!(evs.len(), 3);
        assert_eq!(evs[0].event, "message_start");
        assert_eq!(evs[0].data, "{\"a\":1}");
        assert_eq!(evs[1].data, "{\"b\":2}");
        assert_eq!(evs[2].data, "[DONE]");
    }

    #[tokio::test]
    async fn multiline_data_and_crlf_and_comments() {
        let evs = collect(&[": keep-alive\r\ndata: line1\r\ndata: line2\r\n\r\n"]).await;
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].data, "line1\nline2");
    }

    #[tokio::test]
    async fn json_lines_without_prefix() {
        let evs = collect(&["{\"x\":1}\n{\"x\":2}\n"]).await;
        assert_eq!(evs.len(), 1, "no blank line: flushed at end as one block");
        let evs = collect(&["{\"x\":1}\n\n{\"x\":2}\n\n"]).await;
        assert_eq!(evs.len(), 2);
        assert_eq!(evs[1].data, "{\"x\":2}");
    }
}
