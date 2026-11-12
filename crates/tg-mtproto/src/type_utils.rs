//! `type_utils.h`: Bool is represented directly by Rust bool in tg-scheme.
pub const fn mtp_bool(value: bool) -> bool {
    value
}
pub const fn mtp_is_true(value: bool) -> bool {
    value
}
pub const fn mtp_is_false(value: bool) -> bool {
    !value
}
