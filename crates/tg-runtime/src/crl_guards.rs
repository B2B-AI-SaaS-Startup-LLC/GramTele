//! `crl::guard` (`crl_common_guards.h`, `crl_common_on_main_guarded.h`,
//! `qt/crl_qt_guards.h`).
//!
//! C++ guards an object with a weak pointer that is checked right before the
//! call. Rust objects of the main thread are not `Send`, so a task coming from
//! another thread cannot hold them. Three tools cover the cases:
//!
//! * [`GuardToken`] / [`AsGuard`]: the equivalent of `guard_traits`. Tokens
//!   are weak handles: `Weak<T>`, `rc::Weak<T>`, [`Guard`] from a [`Guardian`]
//!   embedded in an object, [`MainRef<T>`].
//! * [`guard`] and friends wrap a closure; a dead token makes the call a no-op
//!   returning `Default::default()` (the original returns `Return()`).
//! * [`MainRef<T>`]: an id of an `Rc<T>` registered in a per-thread registry
//!   of the main thread. The id is `Send + Sync + Copy`; a background thread
//!   keeps it and posts `main_ref.on_main(|obj| ...)`; the closure receives
//!   the object only on the main thread, only if it is still alive.
//!
//! Limits: `MainRef` objects must be in an `Rc` and registered on the thread
//! that will process the main queue; liveness can only be checked there.

use crate::crl_on_main::{main_queue, MainQueue};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::{Rc, Weak as RcWeak};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

/// `guard_traits::check`.
pub trait GuardToken {
    fn alive(&self) -> bool;
}

/// `guard_traits::create`: makes a token from a borrowed object or pointer.
/// Taking `&self` keeps the borrow out of the closures returned by `guard`.
pub trait AsGuard {
    type Token: GuardToken;
    fn as_guard(&self) -> Self::Token;
}

impl<T> GuardToken for Weak<T> {
    fn alive(&self) -> bool {
        self.strong_count() > 0
    }
}
impl<T> GuardToken for RcWeak<T> {
    fn alive(&self) -> bool {
        self.strong_count() > 0
    }
}
impl<T> AsGuard for Weak<T> {
    type Token = Weak<T>;
    fn as_guard(&self) -> Self::Token {
        self.clone()
    }
}
impl<T> AsGuard for RcWeak<T> {
    type Token = RcWeak<T>;
    fn as_guard(&self) -> Self::Token {
        self.clone()
    }
}
impl<T> AsGuard for Arc<T> {
    type Token = Weak<T>;
    fn as_guard(&self) -> Self::Token {
        Arc::downgrade(self)
    }
}
impl<T> AsGuard for Rc<T> {
    type Token = RcWeak<T>;
    fn as_guard(&self) -> Self::Token {
        Rc::downgrade(self)
    }
}

/// Liveness marker to embed in an object (`base::has_weak_ptr` analogue).
/// Guards made from it die with the object; the object itself may be `!Send`.
#[derive(Default)]
pub struct Guardian {
    alive: Arc<()>,
}

impl Guardian {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn guard(&self) -> Guard {
        Guard {
            weak: Arc::downgrade(&self.alive),
        }
    }
}

/// `Send + Sync + Clone` token of a [`Guardian`].
#[derive(Clone)]
pub struct Guard {
    weak: Weak<()>,
}

impl GuardToken for Guard {
    fn alive(&self) -> bool {
        self.weak.strong_count() > 0
    }
}
impl AsGuard for Guard {
    type Token = Guard;
    fn as_guard(&self) -> Guard {
        self.clone()
    }
}
impl AsGuard for Guardian {
    type Token = Guard;
    fn as_guard(&self) -> Guard {
        self.guard()
    }
}

// ---- guard wrappers ----------------------------------------------------

/// `crl::guard(g, f)` for `FnOnce() -> R`.
pub fn guard<G: AsGuard + ?Sized, R: Default, F: FnOnce() -> R>(g: &G, f: F) -> impl FnOnce() -> R {
    let token = g.as_guard();
    move || if token.alive() { f() } else { R::default() }
}

/// `crl::guard(g, f)` for `FnMut() -> R`.
pub fn guard_mut<G: AsGuard + ?Sized, R: Default, F: FnMut() -> R>(
    g: &G,
    mut f: F,
) -> impl FnMut() -> R {
    let token = g.as_guard();
    move || if token.alive() { f() } else { R::default() }
}

/// `crl::guard(g, f)` for `FnMut(A) -> R`.
pub fn guard_mut1<G: AsGuard + ?Sized, A, R: Default, F: FnMut(A) -> R>(
    g: &G,
    mut f: F,
) -> impl FnMut(A) -> R {
    let token = g.as_guard();
    move |a| if token.alive() { f(a) } else { R::default() }
}

/// `crl::guard(g, f)` for `Fn() -> R` (the result is `Fn` too).
pub fn guard_fn<G: AsGuard + ?Sized, R: Default, F: Fn() -> R>(g: &G, f: F) -> impl Fn() -> R {
    let token = g.as_guard();
    move || if token.alive() { f() } else { R::default() }
}

/// `crl::guard(g, f)` for `Fn(A) -> R`.
pub fn guard_fn1<G: AsGuard + ?Sized, A, R: Default, F: Fn(A) -> R>(
    g: &G,
    f: F,
) -> impl Fn(A) -> R {
    let token = g.as_guard();
    move |a| if token.alive() { f(a) } else { R::default() }
}

/// `crl::guard(g, f)` for `Fn(A, B) -> R`.
pub fn guard_fn2<G: AsGuard + ?Sized, A, B, R: Default, F: Fn(A, B) -> R>(
    g: &G,
    f: F,
) -> impl Fn(A, B) -> R {
    let token = g.as_guard();
    move |a, b| if token.alive() { f(a, b) } else { R::default() }
}

/// `crl::on_main(guard, f)`: token and closure are `Send`; the check runs on
/// the main thread right before the call.
pub fn on_main_guarded<G, F>(g: &G, f: F)
where
    G: AsGuard + ?Sized,
    G::Token: Send + 'static,
    F: FnOnce() + Send + 'static,
{
    let token = g.as_guard();
    main_queue().post(move || {
        if token.alive() {
            f();
        }
    });
}

/// `crl::on_main(guard, f)` from the main thread with non-`Send` parts.
pub fn on_main_local_guarded<G, F>(g: &G, f: F)
where
    G: AsGuard + ?Sized,
    G::Token: 'static,
    F: FnOnce() + 'static,
{
    let token = g.as_guard();
    main_queue().post_local(move || {
        if token.alive() {
            f();
        }
    });
}

// ---- MainRef -----------------------------------------------------------

struct Entry {
    weak: Box<dyn Any>,
    alive: Box<dyn Fn() -> bool>,
}

#[derive(Default)]
struct Registry {
    map: HashMap<u64, Entry>,
    sweep_at: usize,
}

thread_local! {
    static REGISTRY: RefCell<Registry> = RefCell::new(Registry::default());
}

static NEXT_REF: AtomicU64 = AtomicU64::new(1);

/// Identifier of an `Rc<T>` living on the main thread. See the module docs.
pub struct MainRef<T: 'static> {
    id: u64,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for MainRef<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for MainRef<T> {}

impl<T: 'static> MainRef<T> {
    /// Registers `strong` in the registry of the calling (main) thread.
    pub fn new(strong: &Rc<T>) -> Self {
        let id = NEXT_REF.fetch_add(1, Ordering::Relaxed);
        let weak = Rc::downgrade(strong);
        let check = weak.clone();
        REGISTRY.with(|r| {
            let mut r = r.borrow_mut();
            if r.map.len() >= r.sweep_at.max(64) {
                r.map.retain(|_, e| (e.alive)());
                r.sweep_at = r.map.len() * 2;
            }
            r.map.insert(
                id,
                Entry {
                    weak: Box::new(weak),
                    alive: Box::new(move || check.strong_count() > 0),
                },
            );
        });
        MainRef {
            id,
            _marker: PhantomData,
        }
    }

    /// The object, if alive and called on the registering thread.
    pub fn upgrade(&self) -> Option<Rc<T>> {
        REGISTRY.with(|r| {
            r.borrow()
                .map
                .get(&self.id)
                .and_then(|e| e.weak.downcast_ref::<RcWeak<T>>())
                .and_then(RcWeak::upgrade)
        })
    }

    /// Posts `f` to `queue`; it runs on the main thread with the object, or
    /// not at all if the object is gone.
    pub fn on_queue<F: FnOnce(&Rc<T>) + Send + 'static>(&self, queue: &MainQueue, f: F) {
        let me = *self;
        queue.post(move || {
            if let Some(strong) = me.upgrade() {
                f(&strong);
            } else {
                REGISTRY.with(|r| r.borrow_mut().map.remove(&me.id));
            }
        });
    }

    /// `crl::on_main(guard, ...)` for a main-thread object, from any thread.
    pub fn on_main<F: FnOnce(&Rc<T>) + Send + 'static>(&self, f: F) {
        self.on_queue(main_queue(), f);
    }
}

impl<T: 'static> GuardToken for MainRef<T> {
    /// Meaningful on the registering thread only; elsewhere returns false.
    fn alive(&self) -> bool {
        self.upgrade().is_some()
    }
}
impl<T: 'static> AsGuard for MainRef<T> {
    type Token = MainRef<T>;
    fn as_guard(&self) -> Self::Token {
        *self
    }
}
