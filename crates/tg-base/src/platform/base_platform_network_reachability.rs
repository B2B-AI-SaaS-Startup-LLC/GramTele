//! Port of `base/platform/base_platform_network_reachability.h`.

use tg_rpl::Producer;

/// A platform source of the "network is available" state.
pub trait NetworkReachability {
    fn available_value(&self) -> Producer<bool>;
}

/// `NetworkReachability::Create()`.
#[cfg(target_os = "linux")]
pub fn create() -> Option<Box<dyn NetworkReachability>> {
    super::linux::base_network_reachability_linux::create()
}
