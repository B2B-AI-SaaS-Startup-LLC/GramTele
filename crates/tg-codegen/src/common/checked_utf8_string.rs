/// Qt's CheckedUtf8String also rejects a *literal* U+FFFD.
pub fn checked(bytes: &[u8]) -> Option<&str> {
    std::str::from_utf8(bytes)
        .ok()
        .filter(|s| !s.contains('\u{fffd}'))
}
