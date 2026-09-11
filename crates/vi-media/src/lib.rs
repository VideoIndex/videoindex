//! `vi-media`: everything that touches media bytes.
//!
//! - [`acquire`]: the `Acquirer` trait and the `LocalFile` acquirer.
//! - [`probe`]: container and stream facts via libav.
//! - [`frame`]: `Arc<FrameBuffer>` frames with a pixel-format tag.
//! - [`worker`]: the decode worker process. All libav calls happen there, in
//!   a child process, so a crash in a codec never takes the index down.
//! - [`client`]: the parent-side API: [`client::probe`],
//!   [`client::decode_video`], [`client::decode_audio`].
//! - [`protocol`] and [`shm`]: the length-prefixed pipe protocol and the
//!   shared-memory slots frames travel through.
//! - [`sandbox`]: resource limits applied to the worker.
//!
//! libav is reached through the `ffmpeg-next` crate. It was chosen over
//! `rsmpeg` because it wraps the C API in safe Rust types (contexts, frames,
//! packets, scalers, resamplers) so this crate holds almost no `unsafe`, and
//! because its build script detects any libav from 3.0 to 9.0 through
//! pkg-config, which covers the distro ffmpeg 6.1 on azuremc and Homebrew's
//! current ffmpeg on macOS CI without pinning. See `docs/DECISIONS.md`.

#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod acquire;
pub mod client;
pub mod error;
pub mod frame;
pub mod probe;
pub mod protocol;
pub mod sandbox;
pub mod shm;
pub mod worker;

mod decode;

pub use acquire::{Acquired, Acquirer, LocalFile, Source};
pub use client::{decode_audio, decode_video, probe, AudioChunk, AudioStream, FrameStream};
pub use error::{MediaError, Result};
pub use frame::{FrameBuffer, PixelFormat};
pub use probe::{ChapterInfo, Probe, StreamInfo};
pub use protocol::{AudioDecodeRequest, VideoDecodeRequest};

/// Argument that makes the `vi` binary (or any host binary) act as the decode
/// worker. Hosts check `std::env::args().nth(1) == Some(WORKER_ARG)` before
/// normal argument parsing and call [`worker::main`].
pub const WORKER_ARG: &str = "__vi_media_worker";
