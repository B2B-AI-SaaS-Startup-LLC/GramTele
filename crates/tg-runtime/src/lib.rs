//! Port of Telegram Desktop `lib_crl`.
//!
//! `use tg_runtime as crl;` keeps call sites close to the C++ ones. See the
//! crate README for the translation table and the ordering guarantees.

#![forbid(unsafe_code)]

mod crl_async;
mod crl_executor;
mod crl_fp_exceptions;
mod crl_guards;
mod crl_list;
mod crl_main_loop;
mod crl_object_async;
mod crl_object_on_queue;
mod crl_object_on_thread;
mod crl_on_main;
mod crl_on_main_queue;
mod crl_queue;
mod crl_semaphore;
mod crl_time;
mod util;

pub use crl_async::{async_, sync};
pub use crl_executor::Executor;
pub use crl_fp_exceptions::toggle_fp_exceptions;
pub use crl_guards::{
    guard, guard_fn, guard_fn1, guard_fn2, guard_mut, guard_mut1, on_main_guarded,
    on_main_local_guarded, AsGuard, Guard, GuardToken, Guardian, MainRef,
};
pub use crl_main_loop::{LoopQuit, MainLoop};
pub use crl_object_async::{ObjectAsync, Policy, Runner, WeakAsync};
pub use crl_object_on_queue::{ObjectOnQueue, QueuePolicy, WeakOnQueue};
pub use crl_object_on_thread::{ObjectOnThread, ThreadPolicy, WeakOnThread};
pub use crl_on_main::{
    fire_main_update_requests, init_main_queue, main_queue, on_main, on_main_local, on_main_sync,
    on_main_update_requests, process_main_queue, wrap_main_queue, MainQueue,
    UpdateRequestsSubscription, Waker, Wrapper,
};
pub use crl_on_main_queue::{
    on_main_queue, on_main_queue_local, GuardedOnMainQueue, GuardedOnMainQueueLocal,
};
pub use crl_queue::Queue;
pub use crl_semaphore::Semaphore;
pub use crl_time::{adjust_time, init_time, now, profile, ProfileTime, Time};
pub use util::Task;
