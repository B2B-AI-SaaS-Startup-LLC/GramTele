//! Reentrant Data::Flags from data_flags.h. No borrow crosses fire.
use std::{cell::Cell, rc::Rc};
use tg_base::flags::{FlagType, Flags as Bits};
use tg_rpl::{EventStream, Producer};
pub type FlagsUnderlying<E> = <E as FlagType>::Repr;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change<E: FlagType> {
    pub diff: Bits<E>,
    pub value: Bits<E>,
}
pub struct Flags<E: FlagType + 'static> {
    inner: Rc<Inner<E>>,
}
struct Inner<E: FlagType + 'static> {
    value: Cell<Bits<E>>,
    essential: Bits<E>,
    changes: EventStream<Change<E>>,
}
impl<E: FlagType + 'static> Clone for Flags<E> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
impl<E: FlagType + 'static> Default for Flags<E> {
    fn default() -> Self {
        Self::new(Bits::empty())
    }
}
impl<E: FlagType + 'static> Flags<E> {
    pub fn new(value: Bits<E>) -> Self {
        Self::with_essential(value, !Bits::empty())
    }
    pub fn with_essential(value: Bits<E>, essential: Bits<E>) -> Self {
        Self {
            inner: Rc::new(Inner {
                value: Cell::new(value),
                essential,
                changes: EventStream::new(),
            }),
        }
    }
    pub fn set(&self, which: Bits<E>) {
        let diff = which ^ self.current();
        if diff.any() {
            self.inner.value.set(which);
            self.updated(diff);
        }
    }
    pub fn add(&self, which: Bits<E>) {
        let diff = which & !self.current();
        if diff.any() {
            self.inner.value.set(self.current() | which);
            self.updated(diff);
        }
    }
    pub fn remove(&self, which: Bits<E>) {
        let diff = which & self.current();
        if diff.any() {
            self.inner.value.set(self.current() & !which);
            self.updated(diff);
        }
    }
    pub fn current(&self) -> Bits<E> {
        self.inner.value.get()
    }
    pub fn changes(&self) -> Producer<Change<E>> {
        self.inner.changes.events()
    }
    pub fn value(&self) -> Producer<Change<E>> {
        self.inner.changes.events_starting_with(Change {
            diff: self.inner.essential,
            value: self.current(),
        })
    }
    fn updated(&self, diff: Bits<E>) {
        let diff = diff & self.inner.essential;
        if diff.any() {
            self.inner.changes.fire(Change {
                diff,
                value: self.current(),
            });
        }
    }
}
