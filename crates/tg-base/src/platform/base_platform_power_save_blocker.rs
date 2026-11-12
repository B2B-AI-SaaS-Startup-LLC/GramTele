//! Port of `base/platform/base_platform_power_save_blocker.h`: the platform
//! side of [`crate::power_save_blocker`].
//!
//! Linux (`base_power_save_blocker_linux.cpp`: the xdg-desktop-portal
//! `Inhibit` request plus an X11 screensaver reset timer) is deferred, see the
//! docs; a platform layer implements [`PowerSaveBackend`] and installs it with
//! `power_save_blocker::set_backend`.

use crate::power_save_blocker::PowerSaveBlockType;

pub const POWER_SAVE_BLOCK_TYPE_COUNT: usize = PowerSaveBlockType::COUNT;

/// `PowerSaveBlockTypeIndex`.
pub fn power_save_block_type_index(ty: PowerSaveBlockType) -> usize {
    ty as usize
}

/// Window identifier of the window layer (`QPointer<QWindow>` in the
/// original); `None` is a null pointer.
pub type WindowId = Option<u64>;

/// `BlockPowerSave` / `UnblockPowerSave`.
pub trait PowerSaveBackend: Send + Sync {
    fn block(&self, ty: PowerSaveBlockType, description: &str, window: WindowId);
    fn unblock(&self, ty: PowerSaveBlockType, window: WindowId);
}
