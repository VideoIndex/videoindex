//! `vi-core`: types shared by every VideoIndex crate.
//!
//! This crate has no dependency on any other `vi-*` crate. It holds the time
//! model ([`Timestamp`]), identifiers ([`ids`]), the configuration schema
//! ([`Config`]), the error type ([`Error`]), the progress event bus
//! ([`EventBus`]), and the data-model records ([`model`]) described in
//! `docs/04-data-model.md`.

#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod config;
pub mod cpu;
pub mod error;
pub mod event;
pub mod ids;
pub mod model;
pub mod time;

pub use config::Config;
pub use error::{Error, Result};
pub use event::{Event, EventBus, Progress};
pub use ids::*;
pub use time::{TimeRange, Timestamp};
