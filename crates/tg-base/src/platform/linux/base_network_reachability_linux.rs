//! Port of `base_network_reachability_linux.cpp`.

use crate::platform::base_platform_network_reachability::NetworkReachability;

/// Linux has no platform helper (`Create()` returns `nullptr`); the generic
/// code falls back to the toolkit's reachability, which the port does not
/// have yet (see `network_reachability`).
pub fn create() -> Option<Box<dyn NetworkReachability>> {
    None
}
