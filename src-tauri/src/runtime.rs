//! Async runtime owner (DESIGN.md §8, ADR 004).
//!
//! The core owns one explicit multi-threaded Tokio runtime. Extensions schedule
//! work through this interface rather than creating their own runtimes or threads.
//!
//! - `spawn_task`:    cancellable async I/O work with a timeout budget.
//! - `spawn_compute`: CPU-bound work on the blocking thread pool.
//! - `block_on`:      run a future from a synchronous (non-async) caller.
//!
//! `block_on` MUST NOT be called from within an async Tokio context — it panics
//! in that case (the standard Tokio contract). Use it only from extension
//! `activate()` methods, synchronous command handlers, or tests.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

pub struct Runtime {
    rt: Arc<tokio::runtime::Runtime>,
}

impl Runtime {
    pub fn new() -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("nulqor-core")
            .build()
            .expect("failed to start Nulqor async runtime");
        Self { rt: Arc::new(rt) }
    }

    /// Spawn a cancellable async task with a timeout budget.
    /// The task is silently dropped if it exceeds `budget`.
    pub fn spawn_task<F>(&self, budget: Duration, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.rt.spawn(async move {
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
        self.rt.spawn_blocking(job)
    }

    /// Block the calling (non-async) thread until `fut` completes on the
    /// core runtime.  Panics if called from within an async Tokio task.
    pub fn block_on<F: Future>(&self, fut: F) -> F::Output {
        self.rt.block_on(fut)
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
    fn block_on_works_from_sync_context() {
        let rt = Runtime::new();
        let result = rt.block_on(async { 7u32 * 6 });
        assert_eq!(result, 42);
    }
}
