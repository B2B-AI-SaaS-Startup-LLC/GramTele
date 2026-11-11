//! Port of `base/runtime_composer.h`: objects with optional components.
//!
//! In the original `HistoryItem`, `HistoryView::Element`,
//! `HistoryView::Document` and `AbstractLayoutItem` derive
//! `RuntimeComposer<Self>` and carry optional "components" (edited mark,
//! reply, forward info, ...) laid out in one allocation selected by a bit
//! mask. Here the composer is a type-keyed bag:
//!
//! - `item->Has<X>()` -> `composer.has::<X>()`
//! - `item->Get<X>()` -> `composer.get::<X>()` / `get_mut::<X>()` (`Option`)
//! - `AddComponents(X::Bit())` -> `composer.add::<X>()` (default-constructed
//!   unless present; several bits mean several calls)
//! - `RemoveComponents(X::Bit())` -> `composer.remove::<X>()`
//! - `struct X : RuntimeComponent<X, Base>` -> `impl RuntimeComponent<Base>
//!   for X` (see [`runtime_component!`])
//!
//! The 64-component limit and the bit masks are gone. Components present
//! before and after an update keep their value, as in `UpdateComponents`.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;

/// Marks `Self` as a component of `Base` (`RuntimeComponent<Self, Base>`).
pub trait RuntimeComponent<Base>: Any + Default {}

/// Implements [`RuntimeComponent`]: `runtime_component!(Edited, HistoryItem)`.
#[macro_export]
macro_rules! runtime_component {
    ($component:ty, $base:ty) => {
        impl $crate::runtime_composer::RuntimeComponent<$base> for $component {}
    };
}

/// `RuntimeComposer<Base>`. Not `Send`: composed objects are main-thread.
pub struct RuntimeComposer<Base> {
    components: HashMap<TypeId, Box<dyn Any>>,
    _marker: PhantomData<fn() -> Base>,
}

impl<Base> Default for RuntimeComposer<Base> {
    fn default() -> Self {
        Self {
            components: HashMap::new(),
            _marker: PhantomData,
        }
    }
}

impl<Base> std::fmt::Debug for RuntimeComposer<Base> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RuntimeComposer({} components)", self.components.len())
    }
}

impl<Base> RuntimeComposer<Base> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has<T: RuntimeComponent<Base>>(&self) -> bool {
        self.components.contains_key(&TypeId::of::<T>())
    }

    pub fn get<T: RuntimeComponent<Base>>(&self) -> Option<&T> {
        self.components.get(&TypeId::of::<T>())?.downcast_ref()
    }

    pub fn get_mut<T: RuntimeComponent<Base>>(&mut self) -> Option<&mut T> {
        self.components.get_mut(&TypeId::of::<T>())?.downcast_mut()
    }

    /// `AddComponents`: returns true if the component was added (false if
    /// it was already there and is left untouched).
    pub fn add<T: RuntimeComponent<Base>>(&mut self) -> bool {
        use std::collections::hash_map::Entry;
        match self.components.entry(TypeId::of::<T>()) {
            Entry::Occupied(_) => false,
            Entry::Vacant(slot) => {
                slot.insert(Box::new(T::default()));
                true
            }
        }
    }

    /// `AddComponents(X::Bit())` followed by `Get<X>()`.
    pub fn get_or_add<T: RuntimeComponent<Base>>(&mut self) -> &mut T {
        self.add::<T>();
        self.get_mut::<T>().expect("just added")
    }

    /// `RemoveComponents`: returns true if the component was present.
    pub fn remove<T: RuntimeComponent<Base>>(&mut self) -> bool {
        self.components.remove(&TypeId::of::<T>()).is_some()
    }

    /// Number of attached components.
    pub fn len(&self) -> usize {
        self.components.len()
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Item;
    struct Element;

    #[derive(Default, Debug, PartialEq)]
    struct Edited {
        date: i32,
    }
    #[derive(Default)]
    struct Reply {
        id: u64,
    }
    runtime_component!(Edited, Item);
    runtime_component!(Reply, Item);
    runtime_component!(Edited, Element);

    #[test]
    fn add_get_remove() {
        let mut item = RuntimeComposer::<Item>::new();
        assert!(!item.has::<Edited>() && item.get::<Edited>().is_none());
        assert!(item.add::<Edited>());
        assert!(!item.add::<Edited>());
        item.get_mut::<Edited>().unwrap().date = 5;
        assert_eq!(item.get::<Edited>(), Some(&Edited { date: 5 }));
        assert!(!item.has::<Reply>());
        item.get_or_add::<Reply>().id = 7;
        assert_eq!(item.len(), 2);
        assert!(item.remove::<Edited>());
        assert!(!item.remove::<Edited>());
        // Other components survive the update of the set.
        assert_eq!(item.get::<Reply>().map(|r| r.id), Some(7));
    }

    #[test]
    fn re_adding_after_remove_resets_value() {
        let mut item = RuntimeComposer::<Item>::new();
        item.get_or_add::<Edited>().date = 1;
        item.remove::<Edited>();
        assert_eq!(item.get_or_add::<Edited>().date, 0);
    }

    #[test]
    fn bases_are_separate() {
        let mut item = RuntimeComposer::<Item>::new();
        let element = RuntimeComposer::<Element>::new();
        item.add::<Edited>();
        assert!(!element.has::<Edited>());
    }
}
