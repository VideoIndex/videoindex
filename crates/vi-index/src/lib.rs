//! `vi-index`: the [`Storage`] trait and its embedded implementation.
//!
//! The embedded backend is a portable directory (`docs/04-data-model.md`):
//! `manifest.json`, `meta.sqlite` (all tables plus FTS5), `vectors/`
//! (Lance later; a metadata-only stub now), `blobs/` (content-addressed),
//! `cache/operators/`, `jobs/<id>.json`.
//!
//! Pipeline and query code only ever see [`Storage`]; nothing above this
//! crate touches SQLite.

#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod blob;
pub mod embedded;
pub mod error;
pub mod manifest;
pub mod schema;
pub mod storage;
pub mod vectors;

pub use blob::BlobKey;
pub use embedded::EmbeddedIndex;
pub use error::{IndexError, Result};
pub use manifest::{FileHash, Manifest, SCHEMA_VERSION};
pub use storage::{
    Hit, HitKind, IndexStats, Kind, Storage, TextQuery, VectorQuery, VideoStats, Window,
};
