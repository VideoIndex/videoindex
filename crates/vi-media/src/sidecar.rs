//! Sidecar files next to a downloaded video: yt-dlp's `<stem>.info.json`
//! and subtitle files (`<stem>.<lang>.srt`, `<stem>.<lang>.vtt`, `<stem>.srt`).
//! Transfers from a machine that can reach YouTube keep title, chapters and
//! captions this way (deployment notes in `vi_internal`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use vi_core::Timestamp;

use crate::error::{MediaError, Result};

/// A chapter from `.info.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SidecarChapter {
    /// Start.
    pub t0: Timestamp,
    /// End.
    pub t1: Timestamp,
    /// Title.
    pub title: Option<String>,
}

/// The useful subset of a yt-dlp `.info.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InfoJson {
    /// Path of the file.
    pub path: PathBuf,
    /// Site id (`OkEGJ5G3foU`).
    pub id: Option<String>,
    /// Title.
    pub title: Option<String>,
    /// Description.
    pub description: Option<String>,
    /// Channel or uploader name.
    pub channel: Option<String>,
    /// Canonical page URL.
    pub webpage_url: Option<String>,
    /// Upload date.
    pub upload_date: Option<DateTime<Utc>>,
    /// Duration in seconds as reported by the site.
    pub duration_secs: Option<f64>,
    /// Original language, when the site reports it.
    pub language: Option<String>,
    /// Chapters.
    pub chapters: Vec<SidecarChapter>,
    /// Languages for which human-authored subtitles exist.
    pub subtitle_languages: Vec<String>,
    /// Languages for which only automatic captions exist.
    pub auto_caption_languages: Vec<String>,
    /// Playlist id and index when downloaded as part of one.
    pub playlist: Option<(String, u64)>,
    /// Tags.
    pub tags: Vec<String>,
}

impl InfoJson {
    /// Parse a `.info.json` file.
    pub fn read(path: &Path) -> Result<Self> {
        let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
        Ok(Self::from_value(path, &raw))
    }

    /// Parse from the JSON value.
    pub fn from_value(path: &Path, v: &serde_json::Value) -> Self {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
        let upload_date = s("upload_date").and_then(|d| {
            NaiveDate::parse_from_str(&d, "%Y%m%d")
                .ok()
                .and_then(|nd| nd.and_hms_opt(0, 0, 0))
                .map(|ndt| ndt.and_utc())
        });
        let upload_date = upload_date.or_else(|| {
            v.get("timestamp")
                .and_then(|t| t.as_i64())
                .and_then(|t| DateTime::from_timestamp(t, 0))
        });
        let chapters = v
            .get("chapters")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        let t0 = c.get("start_time")?.as_f64()?;
                        let t1 = c.get("end_time")?.as_f64()?;
                        if t1 <= t0 {
                            return None;
                        }
                        Some(SidecarChapter {
                            t0: Timestamp::from_secs_f64(t0, 1000),
                            t1: Timestamp::from_secs_f64(t1, 1000),
                            title: c.get("title").and_then(|t| t.as_str()).map(str::to_string),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let langs = |k: &str| -> Vec<String> {
            v.get(k)
                .and_then(|m| m.as_object())
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default()
        };
        let playlist = s("playlist_id").and_then(|pid| {
            v.get("playlist_index")
                .and_then(|i| i.as_u64())
                .map(|i| (pid, i))
        });
        Self {
            path: path.to_path_buf(),
            id: s("id"),
            title: s("title"),
            description: s("description"),
            channel: s("channel").or_else(|| s("uploader")),
            webpage_url: s("webpage_url").or_else(|| s("original_url")),
            upload_date,
            duration_secs: v.get("duration").and_then(|d| d.as_f64()),
            language: s("language"),
            chapters,
            subtitle_languages: langs("subtitles"),
            auto_caption_languages: langs("automatic_captions"),
            playlist,
            tags: v
                .get("tags")
                .and_then(|t| t.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

/// Subtitle file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleFormat {
    /// SubRip.
    Srt,
    /// WebVTT.
    Vtt,
}

/// A subtitle sidecar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleFile {
    /// Path.
    pub path: PathBuf,
    /// Language tag from the file name (`en`, `en-US`, `en-orig`), if any.
    pub language: Option<String>,
    /// Format from the extension.
    pub format: SubtitleFormat,
}

/// A timed caption.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    /// Start.
    pub t0: Timestamp,
    /// End.
    pub t1: Timestamp,
    /// Text with markup removed and lines joined by spaces.
    pub text: String,
}

/// Everything found next to a media file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Sidecars {
    /// The `.info.json`, if present.
    pub info: Option<InfoJson>,
    /// Subtitle files, sorted by path.
    pub subtitles: Vec<SubtitleFile>,
}

impl Sidecars {
    /// Paths of every sidecar, for moving them with the media.
    pub fn paths(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = self.subtitles.iter().map(|s| s.path.clone()).collect();
        if let Some(i) = &self.info {
            v.push(i.path.clone());
        }
        v
    }
}

/// Find sidecars for `media`: `<stem>.info.json` and `<stem>[.lang].{srt,vtt}`.
pub fn find(media: &Path) -> Sidecars {
    let Some(stem) = media.file_stem().and_then(|s| s.to_str()) else {
        return Sidecars::default();
    };
    let Some(dir) = media.parent() else {
        return Sidecars::default();
    };
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    let mut out = Sidecars::default();
    let info_path = dir.join(format!("{stem}.info.json"));
    if info_path.is_file() {
        match InfoJson::read(&info_path) {
            Ok(i) => out.info = Some(i),
            Err(e) => tracing::warn!("ignoring unreadable {}: {e}", info_path.display()),
        }
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in rd.filter_map(|e| e.ok()) {
        let p = entry.path();
        let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(rest) = name.strip_prefix(stem) else {
            continue;
        };
        let format = match p.extension().and_then(|e| e.to_str()) {
            Some("srt") => SubtitleFormat::Srt,
            Some("vtt") => SubtitleFormat::Vtt,
            _ => continue,
        };
        // rest is ".srt" or ".en.srt" or ".en-US.vtt"
        let middle = rest
            .trim_start_matches('.')
            .rsplit_once('.')
            .map(|(lang, _ext)| lang.to_string())
            .filter(|l| !l.is_empty());
        if rest.matches('.').count() > 2 {
            // "<stem>.something.else.en.srt" is a different file sharing a prefix.
            continue;
        }
        out.subtitles.push(SubtitleFile {
            path: p,
            language: middle,
            format,
        });
    }
    out.subtitles.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Parse a subtitle file into cues.
pub fn parse_file(file: &SubtitleFile) -> Result<Vec<Cue>> {
    let text = std::fs::read_to_string(&file.path)?;
    Ok(match file.format {
        SubtitleFormat::Srt => parse_srt(&text),
        SubtitleFormat::Vtt => parse_vtt(&text),
    })
}

/// Parse SubRip text.
pub fn parse_srt(text: &str) -> Vec<Cue> {
    parse_blocks(text, false)
}

/// Parse WebVTT text.
pub fn parse_vtt(text: &str) -> Vec<Cue> {
    parse_blocks(text, true)
}

fn parse_blocks(text: &str, vtt: bool) -> Vec<Cue> {
    let text = text.trim_start_matches('\u{feff}');
    let mut cues = Vec::new();
    for block in text.replace("\r\n", "\n").split("\n\n") {
        let mut lines = block
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .peekable();
        // Skip VTT headers and notes.
        if vtt {
            if let Some(first) = lines.peek() {
                if first.starts_with("WEBVTT")
                    || first.starts_with("NOTE")
                    || first.starts_with("STYLE")
                    || first.starts_with("REGION")
                {
                    continue;
                }
            }
        }
        let mut timing: Option<(Timestamp, Timestamp)> = None;
        let mut body = Vec::new();
        for line in lines {
            if timing.is_none() {
                if let Some(t) = parse_timing_line(line) {
                    timing = Some(t);
                }
                // Cue identifiers / sequence numbers are skipped.
                continue;
            }
            body.push(strip_markup(line));
        }
        if let Some((t0, t1)) = timing {
            let joined = body
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if !joined.is_empty() && t1 > t0 {
                cues.push(Cue {
                    t0,
                    t1,
                    text: joined,
                });
            }
        }
    }
    dedup_rolling_captions(cues)
}

/// `00:01:02,345 --> 00:01:04,000` or `01:02.345 --> 01:04.000 position:10%`.
fn parse_timing_line(line: &str) -> Option<(Timestamp, Timestamp)> {
    let (a, b) = line.split_once("-->")?;
    let t0 = parse_time(a.trim())?;
    let t1 = parse_time(b.split_whitespace().next()?)?;
    Some((t0, t1))
}

fn parse_time(s: &str) -> Option<Timestamp> {
    let s = s.replace(',', ".");
    let mut parts: Vec<&str> = s.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    let sec_part = parts.pop()?;
    let (secs, frac) = match sec_part.split_once('.') {
        Some((s, f)) => (s.parse::<i64>().ok()?, f),
        None => (sec_part.parse::<i64>().ok()?, ""),
    };
    let ms: i64 = if frac.is_empty() {
        0
    } else {
        let f: String = frac.chars().take(3).collect();
        let n = f.parse::<i64>().ok()?;
        n * 10i64.pow(3 - f.len() as u32)
    };
    let mut total = secs * 1000 + ms;
    let mut mult = 60_000i64;
    for p in parts.iter().rev() {
        total += p.parse::<i64>().ok()? * mult;
        mult *= 60;
    }
    Some(Timestamp::new(total, 1000))
}

/// Remove `<i>`, `<c.color>`, `<00:00:01.000>` word timings, `{\an8}` and
/// HTML entities.
fn strip_markup(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_tag = false;
    let mut in_brace = false;
    for ch in line.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            '{' => in_brace = true,
            '}' if in_brace => in_brace = false,
            _ if in_tag || in_brace => {}
            _ => out.push(ch),
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .trim()
        .to_string()
}

/// YouTube auto-captions arrive as rolling two-line cues: `A`, `A B`, `B`,
/// `B C`, `C`, ... where each cue repeats the previous line. Collapse a cue
/// that extends the previous one into the new material only, treat a cue
/// that is the tail of the previous one as a repeat (extend its end time),
/// and drop exact repeats.
fn dedup_rolling_captions(cues: Vec<Cue>) -> Vec<Cue> {
    let mut out: Vec<Cue> = Vec::with_capacity(cues.len());
    // Full text of the previous input cue (before trimming), for comparison.
    let mut prev_full: Option<String> = None;
    for cue in cues {
        if let (Some(full), Some(prev)) = (&prev_full, out.last_mut()) {
            if *full == cue.text {
                prev.t1 = prev.t1.max(cue.t1);
                continue;
            }
            // The cue is the second line of the previous two-line cue.
            if full.len() > cue.text.len()
                && full.ends_with(cue.text.as_str())
                && full[..full.len() - cue.text.len()].ends_with(' ')
            {
                prev.t1 = prev.t1.max(cue.t1);
                prev_full = Some(cue.text.clone());
                continue;
            }
            if let Some(rest) = cue.text.strip_prefix(full.as_str()) {
                let rest = rest.trim();
                prev_full = Some(cue.text.clone());
                if !rest.is_empty() {
                    out.push(Cue {
                        t0: cue.t0,
                        t1: cue.t1,
                        text: rest.to_string(),
                    });
                }
                continue;
            }
        }
        prev_full = Some(cue.text.clone());
        out.push(cue);
    }
    out
}

/// Group consecutive cues into spans of roughly `target_secs` for retrieval
/// (short cues are too small to search well). Gaps longer than `gap_secs`
/// start a new span.
pub fn group_cues(cues: &[Cue], target_secs: f64, gap_secs: f64) -> Vec<Cue> {
    let mut out: Vec<Cue> = Vec::new();
    for c in cues {
        let merge = match out.last() {
            Some(prev) => {
                let span_len = c.t1.as_secs_f64() - prev.t0.as_secs_f64();
                let gap = c.t0.as_secs_f64() - prev.t1.as_secs_f64();
                span_len <= target_secs && gap <= gap_secs
            }
            None => false,
        };
        if merge {
            if let Some(prev) = out.last_mut() {
                prev.t1 = prev.t1.max(c.t1);
                prev.text.push(' ');
                prev.text.push_str(&c.text);
            }
        } else {
            out.push(c.clone());
        }
    }
    out
}

/// Languages keyed for quick lookup.
pub fn language_set(langs: &[String]) -> BTreeMap<String, ()> {
    langs.iter().map(|l| (l.to_ascii_lowercase(), ())).collect()
}

impl MediaError {
    /// Helper for sidecar problems.
    pub fn sidecar(msg: impl Into<String>) -> Self {
        MediaError::Acquire(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_srt_with_markup_and_crlf() {
        let srt = "1\r\n00:00:01,000 --> 00:00:03,500\r\n<i>Hello</i> there\r\nsecond line\r\n\r\n2\r\n00:00:03,500 --> 00:00:05,000\r\n{\\an8}Bye &amp; thanks\r\n\r\n";
        let cues = parse_srt(srt);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].text, "Hello there second line");
        assert_eq!(cues[0].t0, Timestamp::new(1000, 1000));
        assert_eq!(cues[0].t1, Timestamp::new(3500, 1000));
        assert_eq!(cues[1].text, "Bye & thanks");
    }

    #[test]
    fn collapses_youtube_srt_rolling_two_line_captions() {
        let srt = "1\n00:00:14,719 --> 00:00:19,830\n\nleave armed and ready. So,\n\n2\n00:00:19,830 --> 00:00:19,840\nleave armed and ready. So,\n \n\n3\n00:00:19,840 --> 00:00:23,349\nleave armed and ready. So,\nstarting a company, raising VC,\n\n4\n00:00:23,349 --> 00:00:23,359\nstarting a company, raising VC,\n \n\n5\n00:00:23,359 --> 00:00:27,830\nstarting a company, raising VC,\nwhen do you raise VC?\n\n6\n00:00:27,830 --> 00:00:27,840\nwhen do you raise VC?\n \n";
        let cues = parse_srt(srt);
        let texts: Vec<&str> = cues.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(
            texts,
            vec![
                "leave armed and ready. So,",
                "starting a company, raising VC,",
                "when do you raise VC?"
            ],
            "{cues:?}"
        );
        // The repeat cues extend the end time of the line they repeat.
        assert!((cues[0].t1.as_secs_f64() - 19.84).abs() < 1e-6);
        assert!((cues[1].t1.as_secs_f64() - 23.359).abs() < 1e-6);
        assert!((cues[2].t1.as_secs_f64() - 27.84).abs() < 1e-6);
    }

    #[test]
    fn parses_vtt_with_header_settings_and_rolling_captions() {
        let vtt = "WEBVTT\nKind: captions\nLanguage: en\n\n00:00.000 --> 00:02.000 align:start position:0%\nwelcome to the\n\n00:02.000 --> 00:04.000\nwelcome to the workshop\n\n00:04.000 --> 00:06.000\nwelcome to the workshop\n\n01:00:04.000 --> 01:00:06.120\n<00:00:04.100><c>on</c> retrieval\n";
        let cues = parse_vtt(vtt);
        assert_eq!(cues.len(), 3, "{cues:?}");
        assert_eq!(cues[0].text, "welcome to the");
        assert_eq!(cues[1].text, "workshop");
        assert_eq!(
            cues[1].t1,
            Timestamp::new(6000, 1000),
            "exact repeat extends the cue"
        );
        assert_eq!(cues[2].text, "on retrieval");
        assert_eq!(cues[2].t0, Timestamp::new(3_604_000, 1000));
        assert_eq!(cues[2].t1, Timestamp::new(3_606_120, 1000));
    }

    #[test]
    fn groups_cues_into_windows() {
        let cues: Vec<Cue> = (0..10)
            .map(|i| Cue {
                t0: Timestamp::from_secs(i * 4),
                t1: Timestamp::from_secs(i * 4 + 3),
                text: format!("w{i}"),
            })
            .collect();
        let g = group_cues(&cues, 15.0, 2.0);
        assert_eq!(g.len(), 3, "{g:?}");
        assert_eq!(g[0].text, "w0 w1 w2 w3");
        assert_eq!(g[0].t1, Timestamp::from_secs(15));
    }

    #[test]
    fn finds_sidecars_and_reads_info_json() {
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("003-OkEGJ5G3foU.mp4");
        std::fs::write(&media, b"x").unwrap();
        std::fs::write(
            dir.path().join("003-OkEGJ5G3foU.info.json"),
            serde_json::json!({
                "id": "OkEGJ5G3foU",
                "title": "Workshop",
                "channel": "AI Engineer",
                "webpage_url": "https://www.youtube.com/watch?v=OkEGJ5G3foU",
                "upload_date": "20250612",
                "duration": 7200.5,
                "chapters": [
                    {"start_time": 0.0, "end_time": 60.0, "title": "Intro"},
                    {"start_time": 60.0, "end_time": 7200.5, "title": "Main"}
                ],
                "subtitles": {"en": [{"ext": "vtt"}]},
                "automatic_captions": {"en": [{"ext": "vtt"}], "de": [{"ext": "vtt"}]},
                "playlist_id": "PLcfp", "playlist_index": 3
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("003-OkEGJ5G3foU.en.srt"),
            "1\n00:00:00,000 --> 00:00:01,000\nhi\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("003-OkEGJ5G3foU.srt"),
            "1\n00:00:00,000 --> 00:00:01,000\nhi\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("003-OkEGJ5G3foU.extra.en.srt"), "").unwrap();
        std::fs::write(dir.path().join("004-other.en.srt"), "").unwrap();
        let sc = find(&media);
        assert_eq!(sc.paths().len(), 3);
        let info = sc.info.clone().unwrap();
        assert_eq!(info.title.as_deref(), Some("Workshop"));
        assert_eq!(info.channel.as_deref(), Some("AI Engineer"));
        assert_eq!(
            info.upload_date.unwrap().format("%Y-%m-%d").to_string(),
            "2025-06-12"
        );
        assert_eq!(info.chapters.len(), 2);
        assert_eq!(info.chapters[1].t1, Timestamp::new(7_200_500, 1000));
        assert_eq!(info.subtitle_languages, vec!["en"]);
        assert_eq!(info.auto_caption_languages, vec!["de", "en"]);
        assert_eq!(info.playlist, Some(("PLcfp".to_string(), 3)));
        assert_eq!(sc.subtitles.len(), 2, "{:?}", sc.subtitles);
        let langs: Vec<Option<&str>> = sc.subtitles.iter().map(|s| s.language.as_deref()).collect();
        assert!(langs.contains(&Some("en")) && langs.contains(&None));
    }
}
