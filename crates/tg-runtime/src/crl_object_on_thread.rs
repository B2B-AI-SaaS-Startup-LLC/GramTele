//! `crl::object_on_thread<T>` / `crl::weak_on_thread<T>`
//! (`crl_object_on_thread.*`): the object lives on a dedicated thread, so
//! thread-affine resources are safe. The thread exits after the object is
//! destroyed (the original detached it forever).

use crate::crl_object_async::{ObjectAsync, Policy, WeakAsync};
use crate::util::Task;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;

pub struct ThreadPolicy {
    sender: mpsc::Sender<Task>,
}

impl Default for ThreadPolicy {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel::<Task>();
        std::thread::Builder::new()
            .name("crl-object".into())
            .spawn(move || {
                // Ends when the policy (the sender) is dropped and the
                // backlog is drained.
                while let Ok(task) = receiver.recv() {
                    // A panic is reported by the hook; the thread survives.
                    let _ = catch_unwind(AssertUnwindSafe(task));
                }
            })
            .expect("failed to spawn the object thread");
        ThreadPolicy { sender }
    }
}

impl Policy for ThreadPolicy {
    fn async_plain(&self, task: Task) {
        let _ = self.sender.send(task);
    }
}

pub type ObjectOnThread<T> = ObjectAsync<ThreadPolicy, T>;
pub type WeakOnThread<T> = WeakAsync<ThreadPolicy, T>;
