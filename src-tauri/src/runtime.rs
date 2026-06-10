//! Async runtime owner (DESIGN.md §8, ADR 004).
//!
//! The core owns one explicit multi-threaded Tokio runtime. Extensions schedule
//! work through this interface rather than creating their own runtimes or threads.
//!
//! - `spawn_task`:    cancellable async I/O work with a timeout budget.
//! - `spawn_compute`: CPU-bound work on the blocking thread pool.
//! - `block_on`:      run a future from a synchronous (non-async) caller.
//! - `block_on_compat`: like `block_on`, but safe from Tokio worker threads (HTTP API).

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

pub struct Runtime {
    rt: Option<Arc<tokio::runtime::Runtime>>,
}

impl Runtime {
    pub fn new() -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("nulqor-core")
            .build()
            .expect("failed to start Nulqor async runtime");
        Self { rt: Some(Arc::new(rt)) }
    }

    #[inline]
    fn inner(&self) -> &Arc<tokio::runtime::Runtime> {
        self.rt.as_ref().expect("core runtime already dropped")
    }

    /// Spawn a cancellable async task with a timeout budget.
    /// The task is silently dropped if it exceeds `budget`.
    pub fn spawn_task<F>(&self, budget: Duration, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.inner().spawn(async move {
            let _ = tokio::time::timeout(budget, fut).await;
        });
    }

    /// Dispatch CPU-bound work to the blocking thread pool.
    /// Never blocks async worker threads.
    #[allow(dead_code)]
    pub fn spawn_compute<T, F>(&self, job: F) -> tokio::task::JoinHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.inner().spawn_blocking(job)
    }

    /// Block the calling thread until `fut` completes on the core runtime.
    ///
    /// Panics if called from within an async Tokio task on the same runtime.
    /// Prefer [`block_on_compat`](Self::block_on_compat) from command handlers
    /// that may be invoked while the HTTP API (or other async work) is active.
    pub fn block_on<F: Future>(&self, fut: F) -> F::Output {
        self.inner().block_on(fut)
    }

    /// Like `block_on`, but safe when the caller runs on a Tokio worker thread
    /// (e.g. sync command handler invoked from the axum HTTP API).
    ///
    /// Dispatches `block_on` on a short-lived std thread when already inside
    /// an async runtime context.
    pub fn block_on_compat<F, T>(&self, fut: F) -> T
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        if tokio::runtime::Handle::try_current().is_ok() {
            // Called from a Tokio worker (e.g. HTTP API → sync command handler).
            // block_in_place frees the worker so block_on can poll the future.
            tokio::task::block_in_place(|| self.inner().handle().block_on(fut))
        } else {
            self.inner().block_on(fut)
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // Dropping a Tokio runtime from within an async context panics
        // ("Cannot drop a runtime in a context where blocking is not allowed").
        // In production the core runtime is dropped synchronously at shutdown, so
        // this branch is not taken; it only triggers in async tests that own a
        // Runtime. Offload the drop to a standalone thread in that case.
        if let Some(rt) = self.rt.take() {
            if tokio::runtime::Handle::try_current().is_ok() {
                std::thread::spawn(move || drop(rt));
            }
            // Otherwise `rt` drops here, in a synchronous context, as before.
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[tokio::test]
    async fn spawn_task_runs_within_budget() {
        let rt = Runtime::new();
        let done = std::sync::Arc::new(AtomicBool::new(false));
        let d = done.clone();
        rt.spawn_task(Duration::from_secs(5), async move {
            d.store(true, Ordering::SeqCst);
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(done.load(Ordering::SeqCst), "task should have completed");
    }

    #[tokio::test]
    async fn spawn_task_is_cancelled_on_timeout() {
        let rt = Runtime::new();
        let done = std::sync::Arc::new(AtomicBool::new(false));
        let d = done.clone();
        rt.spawn_task(Duration::from_millis(10), async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            d.store(true, Ordering::SeqCst);
        });
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!done.load(Ordering::SeqCst), "timed-out task should not have set done");
    }

    #[tokio::test]
    async fn spawn_compute_runs_job() {
        let rt = Runtime::new();
        let handle = rt.spawn_compute(|| 42u32 + 1);
        let result = handle.await.expect("compute job should complete");
        assert_eq!(result, 43);
    }

    #[test]
    fn block_on_compat_matches_block_on_from_sync() {
        let rt = Runtime::new();
        assert_eq!(rt.block_on_compat(async { 7u32 * 6 }), 42);
    }

    #[test]
    fn block_on_compat_works_from_spawned_async_task() {
        use std::sync::mpsc;
        use std::sync::Arc;
        use std::time::Duration as StdDuration;

        let rt = Arc::new(Runtime::new());
        let (tx, rx) = mpsc::channel();
        let rt2 = rt.clone();
        rt.spawn_task(Duration::from_secs(5), async move {
            let n = rt2.block_on_compat(async { 99_u32 });
            let _ = tx.send(n);
        });
        let got = rx.recv_timeout(StdDuration::from_secs(2)).expect("timed out waiting for result");
        assert_eq!(got, 99);
    }
}
