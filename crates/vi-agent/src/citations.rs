//! Inline citation markers in the model's text: `[[cite:VIDEO_ID:T0-T1]]`.
//! Models also group several in one pair of outer brackets,
//! `[[cite:A:189-191], [cite:A:204-218]]` or `[[cite:A:1-2; cite:B:3-4]]`;
//! each item becomes its own citation. The scanner works on a token stream,
//! holding back text that might be the start of a marker until it is
//! confirmed or ruled out.

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
    /// A citation (a grouped marker yields one per item, in order).
    Cite(Cite),
}

/// Streaming scanner.
#[derive(Debug, Default)]
pub struct Scanner {
    held: String,
}

const OPEN: &str = "[[";
const CLOSE: &str = "]]";
/// Longest marker held back while waiting for its `]]`: room for a group of
/// about eight items (a single marker is under 60 bytes).
const MAX_MARKER: usize = 400;

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
                            match parse_markers(&marker) {
                                Some(cs) => out.extend(cs.into_iter().map(Piece::Cite)),
                                None => out.push(Piece::Text(marker)),
                            }
                        }
                        None => {
                            // Incomplete: wait, unless it can no longer be a marker.
                            let body = self.held[OPEN.len()..].trim_start();
                            let body = body.strip_prefix('[').unwrap_or(body).trim_start();
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
    let inner = m.strip_prefix(OPEN)?.strip_suffix(CLOSE)?;
    parse_item(inner)
}

/// Parse a marker that may hold several items: `[[cite:A:1-2]]`,
/// `[[cite:A:1-2], [cite:B:3-4]]`, `[[cite:A:1-2; cite:B:3-4]]`. Items are
/// separated by commas or semicolons and may carry their own single brackets.
/// All or nothing: one malformed item leaves the whole marker as text.
pub fn parse_markers(m: &str) -> Option<Vec<Cite>> {
    let inner = m.strip_prefix(OPEN)?.strip_suffix(CLOSE)?;
    inner
        .split([',', ';'])
        .map(|item| {
            let item = item.trim();
            let item = item.strip_prefix('[').unwrap_or(item);
            parse_item(item.strip_suffix(']').unwrap_or(item))
        })
        .collect()
}

/// One `cite:VID:T0-T1` item, without brackets.
fn parse_item(item: &str) -> Option<Cite> {
    let rest = item.trim().strip_prefix("cite:")?;
    let (vid, times) = rest.split_once(':')?;
    let video_id = VideoId::parse(vid.trim()).ok()?;
    let times = times.trim();
    let (a, b) = match times.rsplit_once(['-', '\u{2013}']) {
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

    fn run(tokens: &[&str]) -> (String, Vec<Cite>) {
        let mut sc = Scanner::default();
        let mut pieces = Vec::new();
        for t in tokens {
            pieces.extend(sc.push(t));
        }
        pieces.extend(sc.finish());
        let mut text = String::new();
        let mut cites = Vec::new();
        for p in pieces {
            match p {
                Piece::Text(t) => text.push_str(&t),
                Piece::Cite(c) => cites.push(c),
            }
        }
        (text, cites)
    }

    fn spans(cites: &[Cite]) -> Vec<(f64, f64)> {
        cites.iter().map(|c| (c.t0, c.t1)).collect()
    }

    #[test]
    fn grouped_marker_yields_one_cite_per_item() {
        // The form Gemini wrote on staging (2026-09-24), split across tokens.
        let vid = VideoId::new();
        let whole =
            format!("Two talks cover it [[cite:{vid}:189-191], [cite:{vid}:204-218]]. Next");
        let (text, cites) = run(&[&whole]);
        assert_eq!(text, "Two talks cover it . Next");
        assert_eq!(spans(&cites), [(189.0, 191.0), (204.0, 218.0)]);
        assert!(cites.iter().all(|c| c.video_id == vid));
        // Every split point gives the same result.
        for cut in 1..whole.len() {
            if !whole.is_char_boundary(cut) {
                continue;
            }
            let (a, b) = whole.split_at(cut);
            let (t, c) = run(&[a, b]);
            assert_eq!(t, "Two talks cover it . Next", "cut at {cut}");
            assert_eq!(spans(&c), [(189.0, 191.0), (204.0, 218.0)], "cut at {cut}");
        }
        // Token by token, as small as the model streams them.
        let (t, c) = run(&whole
            .split_inclusive(|ch: char| ":[],- ".contains(ch))
            .collect::<Vec<_>>());
        assert_eq!(t, "Two talks cover it . Next");
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn grouped_marker_variants() {
        let (a, b) = (VideoId::new(), VideoId::new());
        let (text, cites) = run(&[&format!("x [[cite:{a}:1-2; cite:{b}:00:01:05-00:01:10]] y")]);
        assert_eq!(text, "x  y");
        assert_eq!(spans(&cites), [(1.0, 2.0), (65.0, 70.0)]);
        assert_eq!((cites[0].video_id, cites[1].video_id), (a, b));
        // Leading single bracket inside, en dash, three items, spaces.
        let (text, cites) = run(&[&format!(
            "[[ [cite:{a}:10\u{2013}20] , [cite:{b}:30-40],[cite:{a}:50] ]]"
        )]);
        assert_eq!(text, "");
        assert_eq!(spans(&cites), [(10.0, 20.0), (30.0, 40.0), (50.0, 50.0)]);
        // Eight items still fit under the hold-back limit.
        let group = (0..8)
            .map(|i| format!("[cite:{a}:{}-{}]", i * 10, i * 10 + 5))
            .collect::<Vec<_>>()
            .join(", ");
        let (text, cites) = run(&[&format!("[{group}]")]);
        assert_eq!(text, "");
        assert_eq!(cites.len(), 8);
    }

    #[test]
    fn grouped_marker_with_a_bad_item_stays_text() {
        let a = VideoId::new();
        let m = format!("[[cite:{a}:1-2], [cite:nope:3-4]]");
        let (text, cites) = run(&[&m]);
        assert_eq!(text, m);
        assert!(cites.is_empty());
        // An unclosed group is released as text once it exceeds the hold-back limit.
        let long = format!("[[cite:{a}:1-2], {}", "word ".repeat(100));
        let mut sc = Scanner::default();
        let out = sc.push(&long);
        assert!(!out.is_empty(), "held back {} bytes", long.len());
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
