//! Flat, append-only vector store: one pair of files per embedding model
//! under `vectors/`.
//!
//! - `<model>.vec`: a 32-byte header then `count × dim` little-endian
//!   `f32`, one row per embedding, in insertion order.
//! - `<model>.meta`: 40 bytes per row: embedding id (16), video id (16),
//!   target kind (1), alive flag (1), padding (6). Filtering by video and
//!   skipping deleted rows never touches the vector file.
//!
//! Search is exact brute force over a memory map, parallelised with rayon:
//! about 10 ms per 100k rows of 768 dimensions on a 12-core machine, well
//! under the 50 ms retrieval target for the index sizes M1 to M4 produce.
//! Rows are never rewritten; deletes flip the alive flag and `compact`
//! rewrites the files without dead rows. Lance or usearch can replace this
//! behind the same [`crate::Storage`] trait when approximate search is
//! needed (see the decisions log in `vi_internal`).

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rayon::prelude::*;
use vi_core::model::TargetKind;
use vi_core::{EmbeddingId, VideoId};

use crate::error::{IndexError, Result};

const MAGIC: &[u8; 8] = b"VIVEC001";
const HEADER: u64 = 32;
const META_ROW: u64 = 40;

/// The store.
#[derive(Debug)]
pub struct VectorStore {
    root: PathBuf,
    /// Per-model open handles; created on first write or read.
    tables: Mutex<BTreeMap<String, Table>>,
}

#[derive(Debug)]
struct Table {
    vec_path: PathBuf,
    meta_path: PathBuf,
    dim: u32,
    count: u64,
}

/// One search result.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorHit {
    /// Embedding row id.
    pub embedding: EmbeddingId,
    /// Owning video.
    pub video: VideoId,
    /// What the vector embeds.
    pub kind: TargetKind,
    /// Cosine similarity (vectors are stored normalised).
    pub score: f32,
}

fn model_file(model: &str) -> String {
    // Model names may contain '/' (`org/model`); keep them one file.
    model
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn kind_byte(k: TargetKind) -> u8 {
    match k {
        TargetKind::Segment => 1,
        TargetKind::Frame => 2,
        TargetKind::TranscriptSpan => 3,
        TargetKind::OcrSpan => 4,
        TargetKind::Description => 5,
    }
}

fn kind_from_byte(b: u8) -> Option<TargetKind> {
    Some(match b {
        1 => TargetKind::Segment,
        2 => TargetKind::Frame,
        3 => TargetKind::TranscriptSpan,
        4 => TargetKind::OcrSpan,
        5 => TargetKind::Description,
        _ => return None,
    })
}

impl VectorStore {
    /// Store rooted at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            tables: Mutex::new(BTreeMap::new()),
        }
    }

    /// Root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Ensure the directory exists.
    pub fn init(&self) -> Result<()> {
        std::fs::create_dir_all(&self.root)?;
        Ok(())
    }

    /// Whether no vectors are stored for any model.
    pub fn is_empty(&self) -> bool {
        self.models().map(|m| m.is_empty()).unwrap_or(true)
    }

    /// Models with a table on disk.
    pub fn models(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        if !self.root.is_dir() {
            return Ok(out);
        }
        for e in std::fs::read_dir(&self.root)? {
            let p = e?.path();
            if p.extension().is_some_and(|x| x == "vec") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    out.push(stem.to_string());
                }
            }
        }
        out.sort();
        Ok(out)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, BTreeMap<String, Table>>> {
        self.tables
            .lock()
            .map_err(|_| IndexError::Corrupt("vector store mutex poisoned".into()))
    }

    /// Open (or create when `dim` is given) a table's handles.
    fn table<'a>(
        &self,
        tables: &'a mut BTreeMap<String, Table>,
        model: &str,
        dim: Option<u32>,
    ) -> Result<Option<&'a mut Table>> {
        if !tables.contains_key(model) {
            let name = model_file(model);
            let vec_path = self.root.join(format!("{name}.vec"));
            let meta_path = self.root.join(format!("{name}.meta"));
            let t = if vec_path.is_file() {
                let mut f = File::open(&vec_path)?;
                let mut hdr = [0u8; HEADER as usize];
                f.read_exact(&mut hdr)?;
                if &hdr[0..8] != MAGIC {
                    return Err(IndexError::Corrupt(format!(
                        "{} is not a vector file",
                        vec_path.display()
                    )));
                }
                let file_dim = u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]]);
                let len = f.metadata()?.len();
                let count = (len - HEADER) / (u64::from(file_dim) * 4);
                if let Some(d) = dim {
                    if d != file_dim {
                        return Err(IndexError::Invalid(format!(
                            "model '{model}' has {file_dim}-dim vectors on disk, got {d}"
                        )));
                    }
                }
                Table {
                    vec_path,
                    meta_path,
                    dim: file_dim,
                    count,
                }
            } else {
                let Some(d) = dim else {
                    return Ok(None);
                };
                std::fs::create_dir_all(&self.root)?;
                let mut hdr = [0u8; HEADER as usize];
                hdr[0..8].copy_from_slice(MAGIC);
                hdr[8..12].copy_from_slice(&d.to_le_bytes());
                File::create(&vec_path)?.write_all(&hdr)?;
                File::create(&meta_path)?;
                Table {
                    vec_path,
                    meta_path,
                    dim: d,
                    count: 0,
                }
            };
            tables.insert(model.to_string(), t);
        }
        Ok(tables.get_mut(model))
    }

    /// Append rows. Returns the row index of each. Vectors are L2-normalised
    /// on the way in so search is a dot product.
    pub fn append(
        &self,
        model: &str,
        dim: u32,
        rows: &[(EmbeddingId, VideoId, TargetKind, &[f32])],
    ) -> Result<Vec<u64>> {
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let mut tables = self.lock()?;
        let t = self
            .table(&mut tables, model, Some(dim))?
            .ok_or_else(|| IndexError::Corrupt("table creation failed".into()))?;
        if t.dim != dim {
            return Err(IndexError::Invalid(format!(
                "model '{model}' has {}-dim vectors on disk, got {dim}",
                t.dim
            )));
        }
        let mut vec_bytes = Vec::with_capacity(rows.len() * dim as usize * 4);
        let mut meta_bytes = Vec::with_capacity(rows.len() * META_ROW as usize);
        for (id, video, kind, v) in rows {
            if v.len() != dim as usize {
                return Err(IndexError::Invalid(format!(
                    "vector of {} dims for model '{model}' ({dim} expected)",
                    v.len()
                )));
            }
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            let inv = if norm > 0.0 { 1.0 / norm } else { 0.0 };
            for x in v.iter() {
                vec_bytes.extend_from_slice(&(x * inv).to_le_bytes());
            }
            meta_bytes.extend_from_slice(&id.as_u128().to_le_bytes());
            meta_bytes.extend_from_slice(&video.as_u128().to_le_bytes());
            meta_bytes.push(kind_byte(*kind));
            meta_bytes.push(1);
            meta_bytes.extend_from_slice(&[0u8; 6]);
        }
        let mut vf = OpenOptions::new().append(true).open(&t.vec_path)?;
        vf.write_all(&vec_bytes)?;
        let mut mf = OpenOptions::new().append(true).open(&t.meta_path)?;
        mf.write_all(&meta_bytes)?;
        let first = t.count;
        t.count += rows.len() as u64;
        Ok((first..t.count).collect())
    }

    /// Mark rows dead.
    pub fn tombstone(&self, model: &str, rows: &[u64]) -> Result<u64> {
        if rows.is_empty() {
            return Ok(0);
        }
        let mut tables = self.lock()?;
        let Some(t) = self.table(&mut tables, model, None)? else {
            return Ok(0);
        };
        let mut f = OpenOptions::new().write(true).open(&t.meta_path)?;
        let mut n = 0;
        for r in rows {
            if *r < t.count {
                f.seek(SeekFrom::Start(r * META_ROW + 33))?;
                f.write_all(&[0u8])?;
                n += 1;
            }
        }
        Ok(n)
    }

    /// Row count (including dead rows) for a model, 0 if absent.
    pub fn count(&self, model: &str) -> Result<u64> {
        let mut tables = self.lock()?;
        Ok(self
            .table(&mut tables, model, None)?
            .map(|t| t.count)
            .unwrap_or(0))
    }

    /// Dimensionality of a model's table, if it exists.
    pub fn dim(&self, model: &str) -> Result<Option<u32>> {
        let mut tables = self.lock()?;
        Ok(self.table(&mut tables, model, None)?.map(|t| t.dim))
    }

    /// Exact nearest neighbours by cosine similarity, best first. `videos`
    /// empty means all; `kinds` empty means all.
    pub fn search(
        &self,
        model: &str,
        query: &[f32],
        videos: &[VideoId],
        kinds: &[TargetKind],
        k: usize,
    ) -> Result<Vec<VectorHit>> {
        let (vec_path, meta_path, dim, count) = {
            let mut tables = self.lock()?;
            match self.table(&mut tables, model, None)? {
                Some(t) => (t.vec_path.clone(), t.meta_path.clone(), t.dim, t.count),
                None => return Ok(Vec::new()),
            }
        };
        if count == 0 || k == 0 {
            return Ok(Vec::new());
        }
        if query.len() != dim as usize {
            return Err(IndexError::Invalid(format!(
                "query has {} dims, model '{model}' has {dim}",
                query.len()
            )));
        }
        let qn = query.iter().map(|x| x * x).sum::<f32>().sqrt();
        let q: Vec<f32> = if qn > 0.0 {
            query.iter().map(|x| x / qn).collect()
        } else {
            query.to_vec()
        };
        let vf = File::open(&vec_path)?;
        let mf = File::open(&meta_path)?;
        // SAFETY: the files are only ever appended to or have single bytes
        // patched in place (the alive flag); no byte a reader can observe is
        // repurposed, so a concurrent writer cannot produce a view that is
        // undefined behaviour for `&[u8]` readers. Rows beyond `count` (a
        // writer mid-append) are ignored below.
        let vm = unsafe { memmap2::Mmap::map(&vf)? };
        // SAFETY: same argument as for the vector file: append-only apart
        // from single-byte alive flags, and rows past `count` are ignored.
        let mm = unsafe { memmap2::Mmap::map(&mf)? };
        let usable = count
            .min((vm.len() as u64).saturating_sub(HEADER) / (u64::from(dim) * 4))
            .min(mm.len() as u64 / META_ROW) as usize;
        let video_set: Vec<u128> = videos.iter().map(|v| v.as_u128()).collect();
        let kind_set: Vec<u8> = kinds.iter().map(|k| kind_byte(*k)).collect();
        let d = dim as usize;
        let vec_bytes = &vm[HEADER as usize..];
        let meta = &mm[..];
        let chunk = 4096;
        let mut top: Vec<(f32, usize)> = (0..usable)
            .into_par_iter()
            .chunks(chunk)
            .map(|rows| {
                let mut local: Vec<(f32, usize)> = Vec::with_capacity(k.min(rows.len()));
                for r in rows {
                    let m = &meta[r * META_ROW as usize..(r + 1) * META_ROW as usize];
                    if m[33] == 0 {
                        continue;
                    }
                    if !kind_set.is_empty() && !kind_set.contains(&m[32]) {
                        continue;
                    }
                    if !video_set.is_empty() {
                        let vid = u128::from_le_bytes(m[16..32].try_into().unwrap_or([0; 16]));
                        if !video_set.contains(&vid) {
                            continue;
                        }
                    }
                    let off = r * d * 4;
                    let row = &vec_bytes[off..off + d * 4];
                    let mut dot = 0f32;
                    for (i, qx) in q.iter().enumerate() {
                        let b = &row[i * 4..i * 4 + 4];
                        dot += qx * f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                    }
                    if local.len() < k {
                        local.push((dot, r));
                    } else if let Some((mi, _)) = local
                        .iter()
                        .enumerate()
                        .min_by(|a, b| a.1 .0.total_cmp(&b.1 .0))
                        .map(|(i, v)| (i, *v))
                    {
                        if dot > local[mi].0 {
                            local[mi] = (dot, r);
                        }
                    }
                }
                local
            })
            .flatten()
            .collect();
        top.sort_by(|a, b| b.0.total_cmp(&a.0));
        top.truncate(k);
        Ok(top
            .into_iter()
            .filter_map(|(score, r)| {
                let m = &meta[r * META_ROW as usize..(r + 1) * META_ROW as usize];
                let id = u128::from_le_bytes(m[0..16].try_into().ok()?);
                let vid = u128::from_le_bytes(m[16..32].try_into().ok()?);
                Some(VectorHit {
                    embedding: EmbeddingId(ulid::Ulid(id)),
                    video: VideoId(ulid::Ulid(vid)),
                    kind: kind_from_byte(m[32])?,
                    score,
                })
            })
            .collect())
    }

    /// Read one stored (normalised) vector by row.
    pub fn get(&self, model: &str, row: u64) -> Result<Option<Vec<f32>>> {
        let (vec_path, dim, count) = {
            let mut tables = self.lock()?;
            match self.table(&mut tables, model, None)? {
                Some(t) => (t.vec_path.clone(), t.dim, t.count),
                None => return Ok(None),
            }
        };
        if row >= count {
            return Ok(None);
        }
        let mut f = File::open(&vec_path)?;
        f.seek(SeekFrom::Start(HEADER + row * u64::from(dim) * 4))?;
        let mut buf = vec![0u8; dim as usize * 4];
        f.read_exact(&mut buf)?;
        Ok(Some(
            buf.chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect(),
        ))
    }

    /// Rewrite a model's files without dead rows. Returns the new row index
    /// of every surviving embedding so the caller can update its metadata.
    pub fn compact_model(&self, model: &str) -> Result<BTreeMap<EmbeddingId, u64>> {
        let mut tables = self.lock()?;
        let Some(t) = self.table(&mut tables, model, None)? else {
            return Ok(BTreeMap::new());
        };
        let vec_all = std::fs::read(&t.vec_path)?;
        let meta_all = std::fs::read(&t.meta_path)?;
        let d = t.dim as usize;
        let mut vec_out = vec_all[..HEADER as usize].to_vec();
        let mut meta_out = Vec::with_capacity(meta_all.len());
        let mut map = BTreeMap::new();
        let mut new_row = 0u64;
        for r in 0..t.count as usize {
            let m = &meta_all[r * META_ROW as usize..(r + 1) * META_ROW as usize];
            if m[33] == 0 {
                continue;
            }
            let off = HEADER as usize + r * d * 4;
            vec_out.extend_from_slice(&vec_all[off..off + d * 4]);
            meta_out.extend_from_slice(m);
            let id = u128::from_le_bytes(m[0..16].try_into().unwrap_or([0; 16]));
            map.insert(EmbeddingId(ulid::Ulid(id)), new_row);
            new_row += 1;
        }
        let tmp_v = t.vec_path.with_extension("vec.tmp");
        let tmp_m = t.meta_path.with_extension("meta.tmp");
        std::fs::write(&tmp_v, &vec_out)?;
        std::fs::write(&tmp_m, &meta_out)?;
        std::fs::rename(&tmp_v, &t.vec_path)?;
        std::fs::rename(&tmp_m, &t.meta_path)?;
        t.count = new_row;
        Ok(map)
    }

    /// Bytes on disk across all models.
    pub fn bytes(&self) -> Result<u64> {
        let mut total = 0;
        if self.root.is_dir() {
            for e in std::fs::read_dir(&self.root)? {
                total += e?.metadata()?.len();
            }
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_search_tombstone_compact() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors"));
        assert!(store.is_empty());
        let v1 = VideoId::new();
        let v2 = VideoId::new();
        let ids: Vec<EmbeddingId> = (0..4).map(|_| EmbeddingId::new()).collect();
        let rows: Vec<(EmbeddingId, VideoId, TargetKind, &[f32])> = vec![
            (ids[0], v1, TargetKind::Frame, &[1.0, 0.0, 0.0]),
            (ids[1], v1, TargetKind::TranscriptSpan, &[0.0, 2.0, 0.0]),
            (ids[2], v2, TargetKind::Frame, &[0.7, 0.7, 0.0]),
            (ids[3], v2, TargetKind::Frame, &[0.0, 0.0, 1.0]),
        ];
        assert_eq!(store.append("m", 3, &rows).unwrap(), vec![0, 1, 2, 3]);
        assert_eq!(store.count("m").unwrap(), 4);
        assert_eq!(store.dim("m").unwrap(), Some(3));
        let v1n = store.get("m", 1).unwrap().unwrap();
        assert!(
            (v1n[1] - 1.0).abs() < 1e-6 && v1n[0].abs() < 1e-6,
            "{v1n:?}"
        );
        assert!(store.get("m", 9).unwrap().is_none());
        assert!(!store.is_empty());

        let hits = store.search("m", &[1.0, 0.0, 0.0], &[], &[], 10).unwrap();
        assert_eq!(hits[0].embedding, ids[0]);
        assert!((hits[0].score - 1.0).abs() < 1e-6);
        assert_eq!(hits[1].embedding, ids[2]);
        assert!((hits[1].score - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3);
        assert_eq!(hits.len(), 4);

        // Filters.
        let hits = store.search("m", &[1.0, 0.0, 0.0], &[v2], &[], 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.video == v2));
        let hits = store
            .search(
                "m",
                &[0.0, 1.0, 0.0],
                &[],
                &[TargetKind::TranscriptSpan],
                10,
            )
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, TargetKind::TranscriptSpan);

        // Tombstone then compact.
        assert_eq!(store.tombstone("m", &[0]).unwrap(), 1);
        let hits = store.search("m", &[1.0, 0.0, 0.0], &[], &[], 10).unwrap();
        assert_eq!(hits[0].embedding, ids[2]);
        let map = store.compact_model("m").unwrap();
        assert_eq!(map.len(), 3);
        assert_eq!(map[&ids[1]], 0);
        assert_eq!(store.count("m").unwrap(), 3);
        let hits = store.search("m", &[0.0, 0.0, 1.0], &[], &[], 1).unwrap();
        assert_eq!(hits[0].embedding, ids[3]);

        // Reopen from disk.
        let store2 = VectorStore::new(dir.path().join("vectors"));
        assert_eq!(store2.models().unwrap(), vec!["m".to_string()]);
        assert_eq!(store2.count("m").unwrap(), 3);
        assert!(store2
            .search("nope", &[1.0], &[], &[], 3)
            .unwrap()
            .is_empty());
        assert!(store2.search("m", &[1.0], &[], &[], 3).is_err());
        assert!(store2
            .append(
                "m",
                2,
                &[(EmbeddingId::new(), v1, TargetKind::Frame, &[1.0, 0.0])]
            )
            .is_err());
    }

    #[test]
    fn many_rows_rank_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors"));
        let v = VideoId::new();
        let dim = 64;
        let mut ids = Vec::new();
        let mut rows_data: Vec<Vec<f32>> = Vec::new();
        for i in 0..10_000u32 {
            let mut x = vec![0.01f32; dim];
            x[(i % dim as u32) as usize] = 1.0 + (i as f32) / 20_000.0;
            rows_data.push(x);
            ids.push(EmbeddingId::new());
        }
        let rows: Vec<(EmbeddingId, VideoId, TargetKind, &[f32])> = rows_data
            .iter()
            .enumerate()
            .map(|(i, x)| (ids[i], v, TargetKind::Frame, x.as_slice()))
            .collect();
        for chunk in rows.chunks(1000) {
            store.append("big", dim as u32, chunk).unwrap();
        }
        let mut q = vec![0.0f32; dim];
        q[7] = 1.0;
        let hits = store.search("big", &q, &[], &[], 5).unwrap();
        assert_eq!(hits.len(), 5);
        // Every top hit has its peak at dimension 7.
        for h in &hits {
            let i = ids.iter().position(|x| *x == h.embedding).unwrap();
            assert_eq!(i % dim, 7);
        }
        assert!(hits.windows(2).all(|w| w[0].score >= w[1].score));
    }
}
