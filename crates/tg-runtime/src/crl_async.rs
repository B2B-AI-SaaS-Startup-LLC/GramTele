//! `crl::async` and `crl::sync` (`crl_async.h`, `crl_common_sync.h`).

use crate::crl_executor::Executor;
use crate::util::post_and_wait;

/// `crl::async(f)`: runs `f` on the background pool. A no-op after the
/// executor was shut down (like the Qt backend with a null global pool).
pub fn async_<F: FnOnce() + Send + 'static>(f: F) {
    Executor::global().spawn(f);
}

/// `crl::sync(f)`: runs `f` on the pool and waits for the result.
/// `None` only if the executor was shut down. A panic in `f` is re-raised.
pub fn sync<R: Send + 'static, F: FnOnce() -> R + Send + 'static>(f: F) -> Option<R> {
    post_and_wait(
        |task| {
            Executor::global().spawn(task);
        },
        f,
    )
}
