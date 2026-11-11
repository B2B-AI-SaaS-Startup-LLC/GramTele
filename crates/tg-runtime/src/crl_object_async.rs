//! `object_async` / `weak_async` (`crl_common_object_async.h`): an object
//! that lives on a policy (a serial queue or a dedicated thread). It is
//! constructed on the policy, every access is a task posted to it, and it is
//! destroyed on the policy after all earlier tasks.
//!
//! `T` must be `Send` (it moves between pool threads) but not `Sync`.
//! Closures of `with` are `'static`: the C++ idiom of capturing locals by
//! reference (`[&]`) must become `Arc`s or return values of `with_sync`.
//!
//! The rpl `producer` / `producer_on_main` helpers are deferred until
//! `tg-rpl` exists.

use crate::util::{lock, post_and_wait, Task};
use std::sync::{Arc, Mutex, Weak};

/// Where the tasks of an object run. Must execute tasks in FIFO order, one
/// at a time.
pub trait Policy: Send + Sync + 'static {
    fn async_plain(&self, task: Task);
}

struct Data<P: Policy, T> {
    policy: P,
    slot: Mutex<Option<T>>,
}

impl<P: Policy, T: Send + 'static> Data<P, T> {
    /// Posts `f`; the task owns a strong reference until it has run, so the
    /// last release (and `T`'s destructor) happens on the policy.
    fn run(self: &Arc<Self>, f: impl FnOnce(&Data<P, T>) + Send + 'static) {
        let that = Arc::clone(self);
        self.policy.async_plain(Box::new(move || f(&that)));
    }

    fn with(self: &Arc<Self>, f: impl FnOnce(&mut T) + Send + 'static) {
        self.run(move |data| {
            let mut slot = lock(&data.slot);
            f(slot.as_mut().expect("object was not constructed"));
        });
    }

    fn with_sync<R: Send + 'static>(
        self: &Arc<Self>,
        f: impl FnOnce(&mut T) -> R + Send + 'static,
    ) -> Option<R> {
        let that = Arc::clone(self);
        post_and_wait(
            |task| self.policy.async_plain(task),
            move || {
                let mut slot = lock(&that.slot);
                f(slot.as_mut().expect("object was not constructed"))
            },
        )
    }

    fn destroy<V: Send + 'static>(self: &Arc<Self>, value: V) {
        self.policy.async_plain(Box::new(move || drop(value)));
    }
}

/// `weak_async`: a cloneable, `Send` handle that does not keep the object
/// alive. Tasks posted through it are dropped if the owner is already gone.
pub struct WeakAsync<P: Policy, T: Send + 'static> {
    weak: Weak<Data<P, T>>,
}

impl<P: Policy, T: Send + 'static> Clone for WeakAsync<P, T> {
    fn clone(&self) -> Self {
        WeakAsync {
            weak: self.weak.clone(),
        }
    }
}

impl<P: Policy, T: Send + 'static> Default for WeakAsync<P, T> {
    fn default() -> Self {
        WeakAsync { weak: Weak::new() }
    }
}

impl<P: Policy, T: Send + 'static> WeakAsync<P, T> {
    /// Runs `f` on the object's policy, if the object still exists.
    pub fn with(&self, f: impl FnOnce(&mut T) + Send + 'static) {
        if let Some(strong) = self.weak.upgrade() {
            strong.with(f);
            // Release the temporary strong reference on the policy.
            strong.destroy(Arc::clone(&strong));
        }
    }

    /// Like `with` but waits for the result; `None` if the object is gone.
    /// Deadlocks if called from a task of the same policy.
    pub fn with_sync<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut T) -> R + Send + 'static,
    ) -> Option<R> {
        let strong = self.weak.upgrade()?;
        let result = strong.with_sync(f);
        strong.destroy(Arc::clone(&strong));
        result
    }

    /// Drops `value` on the object's policy (on this thread if it is gone).
    pub fn destroy<V: Send + 'static>(&self, value: V) {
        match self.weak.upgrade() {
            Some(strong) => {
                strong.destroy(value);
                strong.destroy(Arc::clone(&strong));
            }
            None => drop(value),
        }
    }

    /// `weak.runner()`: runs arbitrary closures on the object's policy.
    pub fn runner(&self) -> Runner<P, T> {
        Runner { weak: self.clone() }
    }
}

/// Result of [`WeakAsync::runner`].
pub struct Runner<P: Policy, T: Send + 'static> {
    weak: WeakAsync<P, T>,
}

impl<P: Policy, T: Send + 'static> Clone for Runner<P, T> {
    fn clone(&self) -> Self {
        Runner {
            weak: self.weak.clone(),
        }
    }
}

impl<P: Policy, T: Send + 'static> Runner<P, T> {
    pub fn run(&self, f: impl FnOnce() + Send + 'static) {
        self.weak.with(move |_| f());
    }
}

/// `object_async`: the owner. Dropping it destroys the object on the policy
/// after all tasks already posted.
pub struct ObjectAsync<P: Policy, T: Send + 'static> {
    data: Option<Arc<Data<P, T>>>,
}

impl<P: Policy + Default, T: Send + 'static> ObjectAsync<P, T> {
    /// Constructs the object on the policy by calling `ctor` there.
    pub fn new(ctor: impl FnOnce() -> T + Send + 'static) -> Self {
        Self::with_policy(P::default(), ctor)
    }

    /// Like `new`, the constructor gets a weak handle to the object itself
    /// (the "constructor with weak" form of the original).
    pub fn new_with_weak(ctor: impl FnOnce(WeakAsync<P, T>) -> T + Send + 'static) -> Self {
        Self::with_policy_weak(P::default(), ctor)
    }

    /// Moves an already built value in.
    pub fn from_value(value: T) -> Self {
        Self::new(move || value)
    }
}

impl<P: Policy, T: Send + 'static> ObjectAsync<P, T> {
    pub fn with_policy(policy: P, ctor: impl FnOnce() -> T + Send + 'static) -> Self {
        Self::with_policy_weak(policy, move |_| ctor())
    }

    pub fn with_policy_weak(
        policy: P,
        ctor: impl FnOnce(WeakAsync<P, T>) -> T + Send + 'static,
    ) -> Self {
        let data = Arc::new(Data {
            policy,
            slot: Mutex::new(None),
        });
        let weak = WeakAsync {
            weak: Arc::downgrade(&data),
        };
        data.run(move |data| {
            let value = ctor(weak);
            *lock(&data.slot) = Some(value);
        });
        ObjectAsync { data: Some(data) }
    }

    fn data(&self) -> &Arc<Data<P, T>> {
        self.data.as_ref().expect("object is alive until drop")
    }

    pub fn with(&self, f: impl FnOnce(&mut T) + Send + 'static) {
        self.data().with(f);
    }

    /// Runs `f` on the policy and returns its result. Deadlocks if called
    /// from a task of the same policy.
    pub fn with_sync<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut T) -> R + Send + 'static,
    ) -> Option<R> {
        self.data().with_sync(f)
    }

    pub fn destroy<V: Send + 'static>(&self, value: V) {
        self.data().destroy(value);
    }

    pub fn weak(&self) -> WeakAsync<P, T> {
        WeakAsync {
            weak: Arc::downgrade(self.data()),
        }
    }
}

impl<P: Policy, T: Send + 'static> Drop for ObjectAsync<P, T> {
    fn drop(&mut self) {
        // Hand our reference to the policy so the object dies there.
        if let Some(data) = self.data.take() {
            let queue_ref = Arc::clone(&data);
            data.policy.async_plain(Box::new(move || drop(queue_ref)));
        }
    }
}
