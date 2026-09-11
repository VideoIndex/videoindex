//! Bridge from the tokio runtime to the rayon pool. CPU-bound work
//! (hashing, encoding, resizing) must never run on a runtime thread.

use crate::{Error, Result};

/// Run `f` on the global rayon pool and await its result.
pub async fn run<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    rayon::spawn(move || {
        // The receiver may have been dropped by a cancelled caller; that is
        // not an error for the worker.
        let _ = tx.send(f());
    });
    rx.await
        .map_err(|_| Error::Other("rayon task dropped before completing".into()))
}

/// Number of worker threads in the global rayon pool.
pub fn threads() -> usize {
    rayon::current_num_threads()
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn runs_off_runtime() {
        let v = super::run(|| (1..=10u64).sum::<u64>()).await.unwrap();
        assert_eq!(v, 55);
    }
}
