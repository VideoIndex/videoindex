//! Inline citation markers in the model's text: `[[cite:VIDEO_ID:T0-T1]]`.
//! The scanner works on a token stream, holding back text that might be
//! the start of a marker until it is confirmed or ruled out.

use vi_core::VideoId;

/// A parsed marker.
#[derive(Debug, Clone, PartialEq)]
pub struct Cite {
    /// Video.
    pub video_id: VideoId,
    /// Start, seconds.
    pub t0: f64,
    /// End, seconds.
    pub t1: f64,
}

/// What the scanner emits.
#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    /// Plain text.
    Text(String),
    /// A citation.
    Cite(Cite),
}

/// Streaming scanner.
#[derive(Debug, Default)]
pub struct Scanner {
    held: String,
}

const OPEN: &str = "[[";
const CLOSE: &str = "]]";
const MAX_MARKER: usize = 96;

impl Scanner {
    /// Feed a token; returns the pieces that are now certain.
    pub fn push(&mut self, token: &str) -> Vec<Piece> {
        self.held.push_str(token);
        let mut out = Vec::new();
        loop {
            match self.held.find(OPEN) {
                None => {
                    // Keep a possible partial "[" at the very end.
                    if self.held.ends_with('[') {
                        let keep = self.held.len() - 1;
                        let text: String = self.held.drain(..keep).collect();
                        if !text.is_empty() {
                            out.push(Piece::Text(text));
                        }
                    } else if !self.held.is_empty() {
                        out.push(Piece::Text(std::mem::take(&mut self.held)));
                    }
                    return out;
                }
                Some(start) => {
                    if start > 0 {
                        let text: String = self.held.drain(..start).collect();
                        out.push(Piece::Text(text));
                    }
                    // self.held now starts with "[[".
                    match self.held.find(CLOSE) {
                        Some(end) => {
                            let marker: String = self.held.drain(..end + CLOSE.len()).collect();
                            match parse_marker(&marker) {
                                Some(c) => out.push(Piece::Cite(c)),
                                None => out.push(Piece::Text(marker)),
                            }
                        }
                        None => {
                            // Incomplete: wait, unless it can no longer be a marker.
                            let body = &self.held[OPEN.len()..];
                            let plausible = "cite:".starts_with(&body[..body.len().min(5)])
                                || body.starts_with("cite:");
                            if !plausible || self.held.len() > MAX_MARKER {
                                let text: String = self.held.drain(..OPEN.len()).collect();
                                out.push(Piece::Text(text));
                                continue;
                            }
                            return out;
                        }
                    }
                }
            }
        }
    }

    /// Flush whatever is held at the end of the stream as text.
    pub fn finish(&mut self) -> Vec<Piece> {
        if self.held.is_empty() {
            return Vec::new();
        }
        vec![Piece::Text(std::mem::take(&mut self.held))]
    }
}

/// Parse `[[cite:VID:T0-T1]]` (also accepts `T0` alone and `HH:MM:SS`).
pub fn parse_marker(m: &str) -> Option<Cite> {
    let inner = m.strip_prefix(OPEN)?.strip_suffix(CLOSE)?.trim();
    let rest = inner.strip_prefix("cite:")?;
    let (vid, times) = rest.split_once(':')?;
    let video_id = VideoId::parse(vid.trim()).ok()?;
    let times = times.trim();
    let (a, b) = match times.rsplit_once('-') {
        Some((a, b)) if !a.is_empty() && !b.is_empty() && !a.ends_with(':') => (a, b),
        _ => (times, times),
    };
    let t0 = parse_time(a)?;
    let t1 = parse_time(b)?.max(t0);
    Some(Cite { video_id, t0, t1 })
}

fn parse_time(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches('s');
    if let Ok(v) = s.parse::<f64>() {
        return (v >= 0.0).then_some(v);
    }
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    let mut total = 0.0;
    for p in parts {
        total = total * 60.0 + p.parse::<f64>().ok()?;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_markers_split_across_tokens() {
        let vid = VideoId::new();
        let mut sc = Scanner::default();
        let mut pieces = Vec::new();
        for tok in [
            "The speaker ",
            "says so [[ci",
            &format!("te:{vid}:18"),
            "40-1852.5]] and",
            " more [not a cite] end[",
        ] {
            pieces.extend(sc.push(tok));
        }
        pieces.extend(sc.finish());
        let text: String = pieces
            .iter()
            .filter_map(|p| match p {
                Piece::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "The speaker says so  and more [not a cite] end[");
        let cites: Vec<&Cite> = pieces
            .iter()
            .filter_map(|p| match p {
                Piece::Cite(c) => Some(c),
                _ => None,
            })
            .collect();
        assert_eq!(cites.len(), 1);
        assert_eq!(cites[0].video_id, vid);
        assert_eq!((cites[0].t0, cites[0].t1), (1840.0, 1852.5));
    }

    #[test]
    fn bad_markers_become_text() {
        let mut sc = Scanner::default();
        let mut pieces = sc.push("[[cite:notaulid:1-2]] x [[other]]");
        pieces.extend(sc.finish());
        assert!(pieces.iter().all(|p| matches!(p, Piece::Text(_))));
        let vid = VideoId::new();
        let c = parse_marker(&format!("[[cite:{vid}:00:01:05-00:01:10]]")).unwrap();
        assert_eq!((c.t0, c.t1), (65.0, 70.0));
        let c = parse_marker(&format!("[[cite:{vid}:42]]")).unwrap();
        assert_eq!((c.t0, c.t1), (42.0, 42.0));
    }
}
