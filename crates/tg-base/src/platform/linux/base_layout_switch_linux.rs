//! Port of `base_layout_switch_linux.cpp`.

/// Not supported on Linux.
pub fn switch_keyboard_layout_to_english() -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn unsupported() {
        assert!(!super::switch_keyboard_layout_to_english());
    }
}
