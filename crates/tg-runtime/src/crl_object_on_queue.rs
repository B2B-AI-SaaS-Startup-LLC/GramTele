//! `crl::object_on_queue<T>` / `crl::weak_on_queue<T>` (`crl_object_on_queue.h`).

use crate::crl_object_async::{ObjectAsync, Policy, WeakAsync};
use crate::crl_queue::Queue;
use crate::util::Task;

#[derive(Default)]
pub struct QueuePolicy {
    queue: Queue,
}

impl Policy for QueuePolicy {
    fn async_plain(&self, task: Task) {
        self.queue.post(task);
    }
}

pub type ObjectOnQueue<T> = ObjectAsync<QueuePolicy, T>;
pub type WeakOnQueue<T> = WeakAsync<QueuePolicy, T>;
