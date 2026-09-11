//! SQLite schema v1 and migrations. Every timestamp column comes as a
//! `(num, den, secs)` triple: the rational is the truth, `secs` is for
//! indexing and display.

use rusqlite::Connection;

use crate::error::Result;

/// Migration functions in order; index `i` brings the schema to version `i + 1`.
pub const MIGRATIONS: &[fn(&Connection) -> Result<()>] = &[v1, v2];

/// Schema version recorded in `schema_meta`.
pub fn current_version(conn: &Connection) -> Result<u32> {
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'schema_meta'",
        [],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(0);
    }
    let v: Option<String> = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .ok();
    Ok(v.and_then(|s| s.parse().ok()).unwrap_or(0))
}

/// Apply pending migrations.
pub fn migrate(conn: &Connection) -> Result<u32> {
    let mut v = current_version(conn)?;
    while (v as usize) < MIGRATIONS.len() {
        let tx = conn.unchecked_transaction()?;
        MIGRATIONS[v as usize](&tx)?;
        v += 1;
        tx.execute(
            "INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema_version', ?1)",
            [v.to_string()],
        )?;
        tx.commit()?;
    }
    Ok(v)
}

fn v1(c: &Connection) -> Result<()> {
    c.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS schema_meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE videos (
  id              TEXT PRIMARY KEY,
  source_uri      TEXT NOT NULL,
  content_hash    TEXT NOT NULL,
  title           TEXT,
  description     TEXT,
  channel         TEXT,
  published_at    TEXT,
  duration_num    INTEGER NOT NULL,
  duration_den    INTEGER NOT NULL,
  duration_secs   REAL NOT NULL,
  start_wallclock TEXT,
  probe           TEXT NOT NULL,
  index_state     TEXT NOT NULL,
  created_at      TEXT NOT NULL
);
CREATE INDEX videos_content_hash ON videos(content_hash);

CREATE TABLE tracks (
  id           TEXT PRIMARY KEY,
  video_id     TEXT NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
  kind         TEXT NOT NULL,
  stream_index INTEGER NOT NULL,
  codec        TEXT NOT NULL,
  timebase_num INTEGER NOT NULL,
  timebase_den INTEGER NOT NULL,
  width        INTEGER,
  height       INTEGER,
  fps          REAL,
  sample_rate  INTEGER,
  channels     INTEGER,
  language     TEXT
);
CREATE INDEX tracks_video ON tracks(video_id);

CREATE TABLE provenance (
  id               TEXT PRIMARY KEY,
  operator         TEXT NOT NULL,
  operator_version INTEGER NOT NULL,
  provider         TEXT,
  model            TEXT,
  model_version    TEXT,
  prompt_hash      TEXT,
  params           TEXT NOT NULL,
  created_at       TEXT NOT NULL,
  cost_usd         REAL NOT NULL DEFAULT 0,
  tokens_in        INTEGER NOT NULL DEFAULT 0,
  tokens_out       INTEGER NOT NULL DEFAULT 0,
  latency_ms       INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE segments (
  id                 TEXT PRIMARY KEY,
  video_id           TEXT NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
  level              TEXT NOT NULL,
  parent_id          TEXT,
  t0_num             INTEGER NOT NULL,
  t0_den             INTEGER NOT NULL,
  t0_secs            REAL NOT NULL,
  t1_num             INTEGER NOT NULL,
  t1_den             INTEGER NOT NULL,
  t1_secs            REAL NOT NULL,
  keyframe_sample_id TEXT,
  title              TEXT,
  summary            TEXT,
  provenance_id      TEXT NOT NULL
);
CREATE INDEX segments_video_level_t0 ON segments(video_id, level, t0_secs);

CREATE TABLE frame_samples (
  id             TEXT PRIMARY KEY,
  track_id       TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  t_num          INTEGER NOT NULL,
  t_den          INTEGER NOT NULL,
  t_secs         REAL NOT NULL,
  pts            INTEGER NOT NULL,
  is_keyframe    INTEGER NOT NULL,
  phash          INTEGER,
  thumbnail_blob TEXT,
  width          INTEGER NOT NULL,
  height         INTEGER NOT NULL
);
CREATE INDEX frame_samples_track_t ON frame_samples(track_id, t_secs);

CREATE TABLE transcript_spans (
  id            TEXT PRIMARY KEY,
  track_id      TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  t0_num        INTEGER NOT NULL,
  t0_den        INTEGER NOT NULL,
  t0_secs       REAL NOT NULL,
  t1_num        INTEGER NOT NULL,
  t1_den        INTEGER NOT NULL,
  t1_secs       REAL NOT NULL,
  text          TEXT NOT NULL,
  speaker       TEXT,
  language      TEXT,
  confidence    REAL,
  words         TEXT,
  provenance_id TEXT NOT NULL
);
CREATE INDEX transcript_spans_track_t0 ON transcript_spans(track_id, t0_secs);
CREATE VIRTUAL TABLE transcript_fts USING fts5(
  text, content='transcript_spans', content_rowid='rowid',
  tokenize='unicode61 remove_diacritics 2', prefix='2 3'
);
CREATE TRIGGER transcript_ai AFTER INSERT ON transcript_spans BEGIN
  INSERT INTO transcript_fts(rowid, text) VALUES (new.rowid, new.text);
END;
CREATE TRIGGER transcript_ad AFTER DELETE ON transcript_spans BEGIN
  INSERT INTO transcript_fts(transcript_fts, rowid, text) VALUES ('delete', old.rowid, old.text);
END;
CREATE TRIGGER transcript_au AFTER UPDATE ON transcript_spans BEGIN
  INSERT INTO transcript_fts(transcript_fts, rowid, text) VALUES ('delete', old.rowid, old.text);
  INSERT INTO transcript_fts(rowid, text) VALUES (new.rowid, new.text);
END;

CREATE TABLE ocr_spans (
  id              TEXT PRIMARY KEY,
  frame_sample_id TEXT NOT NULL REFERENCES frame_samples(id) ON DELETE CASCADE,
  t_num           INTEGER NOT NULL,
  t_den           INTEGER NOT NULL,
  t_secs          REAL NOT NULL,
  text            TEXT NOT NULL,
  bbox_x          REAL,
  bbox_y          REAL,
  bbox_w          REAL,
  bbox_h          REAL,
  confidence      REAL,
  provenance_id   TEXT NOT NULL
);
CREATE INDEX ocr_spans_frame ON ocr_spans(frame_sample_id);
CREATE INDEX ocr_spans_t ON ocr_spans(t_secs);
CREATE VIRTUAL TABLE ocr_fts USING fts5(
  text, content='ocr_spans', content_rowid='rowid',
  tokenize='unicode61 remove_diacritics 2', prefix='2 3'
);
CREATE TRIGGER ocr_ai AFTER INSERT ON ocr_spans BEGIN
  INSERT INTO ocr_fts(rowid, text) VALUES (new.rowid, new.text);
END;
CREATE TRIGGER ocr_ad AFTER DELETE ON ocr_spans BEGIN
  INSERT INTO ocr_fts(ocr_fts, rowid, text) VALUES ('delete', old.rowid, old.text);
END;
CREATE TRIGGER ocr_au AFTER UPDATE ON ocr_spans BEGIN
  INSERT INTO ocr_fts(ocr_fts, rowid, text) VALUES ('delete', old.rowid, old.text);
  INSERT INTO ocr_fts(rowid, text) VALUES (new.rowid, new.text);
END;

CREATE TABLE descriptions (
  id            TEXT PRIMARY KEY,
  target_kind   TEXT NOT NULL,
  target_id     TEXT NOT NULL,
  kind          TEXT NOT NULL,
  text          TEXT NOT NULL,
  structured    TEXT,
  provenance_id TEXT NOT NULL
);
CREATE INDEX descriptions_target ON descriptions(target_kind, target_id);
CREATE VIRTUAL TABLE descriptions_fts USING fts5(
  text, content='descriptions', content_rowid='rowid',
  tokenize='unicode61 remove_diacritics 2', prefix='2 3'
);
CREATE TRIGGER descriptions_ai AFTER INSERT ON descriptions BEGIN
  INSERT INTO descriptions_fts(rowid, text) VALUES (new.rowid, new.text);
END;
CREATE TRIGGER descriptions_ad AFTER DELETE ON descriptions BEGIN
  INSERT INTO descriptions_fts(descriptions_fts, rowid, text) VALUES ('delete', old.rowid, old.text);
END;
CREATE TRIGGER descriptions_au AFTER UPDATE ON descriptions BEGIN
  INSERT INTO descriptions_fts(descriptions_fts, rowid, text) VALUES ('delete', old.rowid, old.text);
  INSERT INTO descriptions_fts(rowid, text) VALUES (new.rowid, new.text);
END;

CREATE TABLE entities (
  id             TEXT PRIMARY KEY,
  video_id       TEXT NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
  kind           TEXT NOT NULL,
  name           TEXT NOT NULL,
  canonical_name TEXT NOT NULL,
  attributes     TEXT NOT NULL
);
CREATE INDEX entities_video ON entities(video_id, canonical_name);

CREATE TABLE entity_mentions (
  entity_id   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
  t0_num      INTEGER NOT NULL,
  t0_den      INTEGER NOT NULL,
  t0_secs     REAL NOT NULL,
  t1_num      INTEGER NOT NULL,
  t1_den      INTEGER NOT NULL,
  t1_secs     REAL NOT NULL,
  source_kind TEXT NOT NULL,
  source_id   TEXT NOT NULL,
  confidence  REAL
);
CREATE INDEX entity_mentions_entity ON entity_mentions(entity_id, t0_secs);

CREATE TABLE events (
  id            TEXT PRIMARY KEY,
  video_id      TEXT NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
  t0_num        INTEGER NOT NULL,
  t0_den        INTEGER NOT NULL,
  t0_secs       REAL NOT NULL,
  t1_num        INTEGER NOT NULL,
  t1_den        INTEGER NOT NULL,
  t1_secs       REAL NOT NULL,
  text          TEXT NOT NULL,
  participants  TEXT NOT NULL,
  provenance_id TEXT NOT NULL
);
CREATE INDEX events_video_t0 ON events(video_id, t0_secs);

CREATE TABLE embeddings (
  id            TEXT PRIMARY KEY,
  target_kind   TEXT NOT NULL,
  target_id     TEXT NOT NULL,
  model         TEXT NOT NULL,
  dim           INTEGER NOT NULL,
  provenance_id TEXT NOT NULL,
  UNIQUE(target_kind, target_id, model)
);

CREATE TABLE sessions (
  id         TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  state      TEXT NOT NULL
);
"#,
    )?;
    Ok(())
}

/// v2: embeddings know their row in the model's vector file.
fn v2(c: &Connection) -> Result<()> {
    c.execute_batch(
        r#"
ALTER TABLE embeddings ADD COLUMN row INTEGER;
CREATE INDEX IF NOT EXISTS embeddings_target ON embeddings(target_kind, target_id);
CREATE INDEX IF NOT EXISTS embeddings_model_row ON embeddings(model, row);
"#,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_fresh_db_to_current_version() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(current_version(&conn).unwrap(), 0);
        assert_eq!(migrate(&conn).unwrap(), crate::SCHEMA_VERSION);
        assert_eq!(current_version(&conn).unwrap(), crate::SCHEMA_VERSION);
        // Idempotent.
        assert_eq!(migrate(&conn).unwrap(), crate::SCHEMA_VERSION);
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name LIKE '%_fts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 3, "three FTS5 tables");
    }
}
