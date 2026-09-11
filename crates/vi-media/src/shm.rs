//! Shared-memory slots between the worker and the parent.
//!
//! The parent creates a file-backed region divided into fixed-size slots and
//! tells the worker its path. The worker maps it writable and fills one slot
//! per frame; the parent maps it read-only and wraps each announced slot in a
//! [`SlotGuard`]. Ownership of a slot moves with protocol messages: the worker
//! owns a slot until it sends `Frame { slot }`, the parent owns it until the
//! guard drops and a `SlotFree { slot }` message goes back. No slot is ever
//! written by one side while the other holds it, which is what makes the
//! mappings below sound.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use memmap2::{Mmap, MmapMut};
use tokio::sync::mpsc;

use crate::error::{MediaError, Result};

/// Geometry of a shared region, sent to the worker in the request.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShmSpec {
    /// Path of the backing file.
    pub path: PathBuf,
    /// Bytes per slot.
    pub slot_size: usize,
    /// Number of slots.
    pub slots: u32,
}

impl ShmSpec {
    /// Total bytes.
    pub fn total_len(&self) -> usize {
        self.slot_size * self.slots as usize
    }
}

/// Read-only mapping held by the parent.
pub struct SharedRegion {
    spec: ShmSpec,
    map: Mmap,
    /// Kept so the file is removed when the region drops.
    _file: Option<tempfile::NamedTempFile>,
}

impl std::fmt::Debug for SharedRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedRegion")
            .field("spec", &self.spec)
            .finish()
    }
}

impl SharedRegion {
    /// Create the backing file and map it read-only. The file is deleted when
    /// the returned region drops; the worker must have opened it by then,
    /// which the protocol guarantees (it opens the file before replying).
    pub fn create(slot_size: usize, slots: u32) -> Result<Self> {
        if slot_size == 0 || slots == 0 {
            return Err(MediaError::Invalid(
                "shm slot_size and slots must be > 0".into(),
            ));
        }
        let dir = shm_dir();
        std::fs::create_dir_all(&dir)?;
        let file = tempfile::Builder::new()
            .prefix("vi-shm-")
            .suffix(".bin")
            .tempfile_in(&dir)?;
        let total = slot_size
            .checked_mul(slots as usize)
            .ok_or_else(|| MediaError::Invalid("shm region too large".into()))?;
        file.as_file().set_len(total as u64)?;
        // SAFETY: the mapping is read-only and backed by a file we just
        // created and sized. The only other writer is the worker, and the slot
        // ownership protocol described in the module docs guarantees the
        // worker never writes a slot while the parent reads it. The file
        // cannot shrink: neither side calls truncate after this point.
        let map = unsafe { Mmap::map(file.as_file())? };
        Ok(Self {
            spec: ShmSpec {
                path: file.path().to_path_buf(),
                slot_size,
                slots,
            },
            map,
            _file: Some(file),
        })
    }

    /// Geometry to send to the worker.
    pub fn spec(&self) -> &ShmSpec {
        &self.spec
    }

    /// Bytes of one slot, up to `len`.
    pub fn slot(&self, slot: u32, len: usize) -> &[u8] {
        let start = slot as usize * self.spec.slot_size;
        let end = (start + len.min(self.spec.slot_size)).min(self.map.len());
        &self.map[start.min(end)..end]
    }
}

/// Writable mapping held by the worker.
pub struct SharedRegionMut {
    spec: ShmSpec,
    map: MmapMut,
}

impl std::fmt::Debug for SharedRegionMut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedRegionMut")
            .field("spec", &self.spec)
            .finish()
    }
}

impl SharedRegionMut {
    /// Open the parent's file for writing.
    pub fn open(spec: &ShmSpec) -> Result<Self> {
        let file: File = OpenOptions::new().read(true).write(true).open(&spec.path)?;
        let len = file.metadata()?.len();
        if len < spec.total_len() as u64 {
            return Err(MediaError::Protocol(format!(
                "shm file {} is {} bytes, expected {}",
                spec.path.display(),
                len,
                spec.total_len()
            )));
        }
        // SAFETY: the file was created and sized by the parent and is never
        // truncated. Writes go only to slots the worker currently owns per the
        // protocol, so the parent's read-only view never observes a torn
        // slot it has been told about.
        let map = unsafe { MmapMut::map_mut(&file)? };
        Ok(Self {
            spec: spec.clone(),
            map,
        })
    }

    /// Writable bytes of one slot.
    pub fn slot_mut(&mut self, slot: u32) -> &mut [u8] {
        let start = slot as usize * self.spec.slot_size;
        let end = start + self.spec.slot_size;
        &mut self.map[start..end]
    }

    /// Geometry.
    pub fn spec(&self) -> &ShmSpec {
        &self.spec
    }
}

/// Parent-side handle to one filled slot. Dropping it returns the slot to
/// the worker.
pub struct SlotGuard {
    region: Arc<SharedRegion>,
    slot: u32,
    len: usize,
    release: mpsc::UnboundedSender<u32>,
}

impl std::fmt::Debug for SlotGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlotGuard")
            .field("slot", &self.slot)
            .field("len", &self.len)
            .finish()
    }
}

impl SlotGuard {
    /// Wrap a slot the worker just announced.
    pub fn new(
        region: Arc<SharedRegion>,
        slot: u32,
        len: usize,
        release: mpsc::UnboundedSender<u32>,
    ) -> Self {
        Self {
            region,
            slot,
            len,
            release,
        }
    }

    /// Slot index.
    pub fn slot(&self) -> u32 {
        self.slot
    }

    /// Valid bytes in the slot.
    pub fn len(&self) -> usize {
        self.len
    }

    /// True when no bytes are valid.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The bytes.
    pub fn as_slice(&self) -> &[u8] {
        self.region.slot(self.slot, self.len)
    }
}

impl Drop for SlotGuard {
    fn drop(&mut self) {
        // The receiver is gone once the session ended; nothing to release to.
        let _ = self.release.send(self.slot);
    }
}

/// Directory for shared-memory files: `/dev/shm` on Linux when available
/// (RAM-backed, no disk I/O), else the temp dir.
pub fn shm_dir() -> PathBuf {
    let dev_shm = Path::new("/dev/shm");
    if cfg!(target_os = "linux") && dev_shm.is_dir() {
        return dev_shm.to_path_buf();
    }
    std::env::temp_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_writes_parent_reads() {
        let region = Arc::new(SharedRegion::create(16, 3).unwrap());
        let mut w = SharedRegionMut::open(region.spec()).unwrap();
        w.slot_mut(1)[..4].copy_from_slice(&[9, 8, 7, 6]);
        assert_eq!(region.slot(1, 4), &[9, 8, 7, 6]);
        let (tx, mut rx) = mpsc::unbounded_channel();
        {
            let g = SlotGuard::new(region.clone(), 1, 4, tx);
            assert_eq!(g.as_slice(), &[9, 8, 7, 6]);
        }
        assert_eq!(rx.try_recv().unwrap(), 1);
    }

    #[test]
    fn rejects_zero_geometry() {
        assert!(SharedRegion::create(0, 1).is_err());
        assert!(SharedRegion::create(1, 0).is_err());
    }
}
