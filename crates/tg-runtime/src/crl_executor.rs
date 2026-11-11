//! Background execution (`crl_async.h` and the dispatch / winapi / Qt / tmc
//! backends).
//!
//! All backends are replaced by the *blocking pool* of a private tokio
//! runtime. crl tasks are plain synchronous closures that may block (disk,
//! semaphores, `sync`), so running them on async workers would starve the
//! reactor; the blocking pool grows on demand (up to 512 threads) and a
//! blocked task never prevents others from running.

use std::sync::{Arc, OnceLock, RwLock};
use tokio::runtime::{Builder, Handle, Runtime};

pub struct Executor {
    runtime: RwLock<Option<Runtime>>,
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor {
    pub fn new() -> Self {
        let runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("crl-pool")
            .build()
            .expect("failed to create the tokio runtime");
        Executor {
            runtime: RwLock::new(Some(runtime)),
        }
    }

    /// The process-wide executor used by `async_`, `sync` and `Queue::new`.
    pub fn global() -> &'static Arc<Executor> {
        static GLOBAL: OnceLock<Arc<Executor>> = OnceLock::new();
        GLOBAL.get_or_init(|| Arc::new(Executor::new()))
    }

    /// Runs `f` on the pool. Returns false (dropping `f`) after shutdown.
    pub fn spawn(&self, f: impl FnOnce() + Send + 'static) -> bool {
        let guard = self.runtime.read().unwrap_or_else(|e| e.into_inner());
        match guard.as_ref() {
            Some(runtime) => {
                drop(runtime.spawn_blocking(f));
                true
            }
            None => false,
        }
    }

    /// Stops accepting work; running tasks finish on their own, queued ones
    /// are dropped unexecuted. Does not wait.
    pub fn shutdown(&self) {
        let runtime = self
            .runtime
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(runtime) = runtime {
            runtime.shutdown_background();
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.runtime
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .is_none()
    }

    /// Handle of the underlying tokio runtime, for crates that need async I/O.
    pub fn handle(&self) -> Option<Handle> {
        self.runtime
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|r| r.handle().clone())
    }
}

impl Drop for Executor {
    fn drop(&mut self) {
        self.shutdown();
    }
}
