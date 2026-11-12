//! Port of `base/power_save_blocker.h` / `power_save_blocker.cpp`.
//!
//! Blockers of one type share one platform request: the first one issues it
//! (`BlockPowerSave`), the one that defined the active request releases it
//! when it goes away, and the request is then re-issued for another blocker
//! (preferring one with a window). Thread-safe, like the original.

use crate::platform::base_platform_power_save_blocker::{PowerSaveBackend, WindowId};
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// `base::PowerSaveBlockType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PowerSaveBlockType {
    PreventAppSuspension,
    PreventDisplaySleep,
}

impl PowerSaveBlockType {
    /// `kCount`.
    pub const COUNT: usize = 2;
}

#[derive(Default)]
struct BlockersSet {
    list: BTreeSet<u64>,
    window: WindowId,
    description: String,
}

#[derive(Default)]
struct Registry {
    backend: Option<Arc<dyn PowerSaveBackend>>,
    sets: HashMap<PowerSaveBlockType, BlockersSet>,
    /// Descriptions and windows of the live blockers.
    info: HashMap<u64, (String, WindowId)>,
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}

fn registry_lock() -> std::sync::MutexGuard<'static, Registry> {
    registry().lock().unwrap_or_else(|e| e.into_inner())
}

/// Installs the platform implementation (`None` makes blocking a no-op).
pub fn set_backend(backend: Option<Arc<dyn PowerSaveBackend>>) {
    registry_lock().backend = backend;
}

/// `base::PowerSaveBlocker`: blocks while alive.
#[derive(Debug)]
pub struct PowerSaveBlocker {
    id: u64,
    ty: PowerSaveBlockType,
    description: String,
    window: WindowId,
}

impl PowerSaveBlocker {
    pub fn new(ty: PowerSaveBlockType, description: &str, window: WindowId) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let mut registry = registry_lock();
        registry.info.insert(id, (description.to_owned(), window));
        let backend = registry.backend.clone();
        let set = registry.sets.entry(ty).or_default();
        if set.list.is_empty() {
            set.description = description.to_owned();
            set.window = window;
            if let Some(backend) = backend {
                backend.block(ty, &set.description, set.window);
            }
        }
        set.list.insert(id);
        Self {
            id,
            ty,
            description: description.to_owned(),
            window,
        }
    }

    pub fn type_(&self) -> PowerSaveBlockType {
        self.ty
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn window(&self) -> WindowId {
        self.window
    }
}

impl Drop for PowerSaveBlocker {
    fn drop(&mut self) {
        let mut registry = registry_lock();
        registry.info.remove(&self.id);
        let backend = registry.backend.clone();
        let Registry { sets, info, .. } = &mut *registry;
        let set = sets.entry(self.ty).or_default();
        set.list.remove(&self.id);
        if set.description != self.description || set.window != self.window {
            return;
        }
        if let Some(backend) = backend.as_ref() {
            backend.unblock(self.ty, self.window);
        }
        if !set.list.is_empty() {
            let good = set
                .list
                .iter()
                .find(|id| info.get(id).is_some_and(|(_, window)| window.is_some()));
            let use_id = good.or_else(|| set.list.iter().next()).copied().unwrap();
            let (description, window) = info.get(&use_id).cloned().unwrap();
            set.description = description;
            set.window = window;
            if let Some(backend) = backend {
                backend.block(self.ty, &set.description, set.window);
            }
        }
    }
}

/// `base::UpdatePowerSaveBlocker`.
pub fn update_power_save_blocker(
    blocker: &mut Option<PowerSaveBlocker>,
    block: bool,
    ty: PowerSaveBlockType,
    description: impl FnOnce() -> String,
    window: impl FnOnce() -> WindowId,
) {
    if block && blocker.is_none() {
        *blocker = Some(PowerSaveBlocker::new(ty, &description(), window()));
    } else if !block && blocker.is_some() {
        *blocker = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder {
        log: Mutex<Vec<String>>,
    }
    impl PowerSaveBackend for Recorder {
        fn block(&self, ty: PowerSaveBlockType, description: &str, window: WindowId) {
            self.log
                .lock()
                .unwrap()
                .push(format!("block {ty:?} {description} {window:?}"));
        }
        fn unblock(&self, ty: PowerSaveBlockType, window: WindowId) {
            self.log
                .lock()
                .unwrap()
                .push(format!("unblock {ty:?} {window:?}"));
        }
    }

    // The registry is process-global: one test drives all scenarios.
    #[test]
    fn shared_request_per_type() {
        use PowerSaveBlockType::*;
        let recorder = Arc::new(Recorder::default());
        set_backend(Some(recorder.clone()));
        let take = || std::mem::take(&mut *recorder.log.lock().unwrap());

        let a = PowerSaveBlocker::new(PreventDisplaySleep, "video", None);
        assert_eq!(take(), vec!["block PreventDisplaySleep video None"]);
        // Same type: no second request.
        let b = PowerSaveBlocker::new(PreventDisplaySleep, "call", Some(7));
        let c = PowerSaveBlocker::new(PreventDisplaySleep, "other", None);
        assert!(take().is_empty());
        // Another type is independent.
        let s = PowerSaveBlocker::new(PreventAppSuspension, "upload", None);
        assert_eq!(take(), vec!["block PreventAppSuspension upload None"]);
        drop(s);
        assert_eq!(take(), vec!["unblock PreventAppSuspension None"]);

        // Removing a blocker that did not define the request does nothing.
        drop(c);
        assert!(take().is_empty());
        // Removing the defining one releases and re-issues for the one with
        // a window.
        drop(a);
        assert_eq!(
            take(),
            vec![
                "unblock PreventDisplaySleep None",
                "block PreventDisplaySleep call Some(7)"
            ]
        );
        drop(b);
        assert_eq!(take(), vec!["unblock PreventDisplaySleep Some(7)"]);

        // update_power_save_blocker.
        let mut slot = None;
        update_power_save_blocker(&mut slot, true, PreventAppSuspension, || "x".into(), || None);
        assert!(slot.is_some());
        update_power_save_blocker(&mut slot, true, PreventAppSuspension, || "y".into(), || None);
        assert_eq!(slot.as_ref().unwrap().description(), "x");
        update_power_save_blocker(&mut slot, false, PreventAppSuspension, || "".into(), || None);
        assert!(slot.is_none());
        take();
        set_backend(None);
    }
}
