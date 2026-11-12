//! Serialization-level subset of `mtproto/core_types.h` and
//! `mtproto/type_utils.h`.
//!
//! These are the helpers the whole client calls when building and reading
//! TL values (`MTP_int`, `MTP_string`, `qs`, `MTP_bool`, ...). They are thin
//! wrappers over the [`crate::tl`] functions; the `mtproto` port is expected
//! to re-export this module from its own `core_types` instead of redefining
//! it. Everything else in `core_types.h` (`mtpRequestId`, `MTP::DcId`, dc
//! shifts, `mtpLayers`) belongs to the `mtproto` port.
//!
//! Most of them are identities: in the port `MTPint` is `i32`, `MTPlong` is
//! `u64`, `MTPstring` is `Vec<u8>`, `MTPVector<T>` is `Vec<T>` and `MTPBool`
//! is `bool`, so `MTP_int(x)` can simply be written as `x`.

use crate::flags::Flags;
use crate::tl::tl_basic_types::{self as basic, Int128, Int256, Prime, TypeId};

/// `mtpPrime`.
pub type MtpPrime = Prime;
/// `mtpTypeId`.
pub type MtpTypeId = TypeId;
/// `mtpBuffer`.
pub type MtpBuffer = basic::Buffer;

/// `mtpc_rpc_result` (parsed manually, not in the generated scheme).
pub const ID_RPC_RESULT: TypeId = 0xf35c_6d01;
/// `mtpc_msg_container` (parsed manually).
pub const ID_MSG_CONTAINER: TypeId = 0x73f1_f8dc;
/// `mtpc_gzip_packed` (parsed manually; unpacking is done by the MTProto
/// session, the generated readers never see it, as in the original).
pub const ID_GZIP_PACKED: TypeId = 0x3072_cfa1;
/// `mtpc_core_message` (`static const mtpTypeId mtpc_core_message = -1`).
pub const ID_CORE_MESSAGE: TypeId = u32::MAX;

/// `MTP_int`.
pub const fn mtp_int(v: i32) -> i32 {
    v
}

/// `MTP_long`.
pub const fn mtp_long(v: u64) -> u64 {
    v
}

/// `MTP_int128`.
pub const fn mtp_int128(l: u64, h: u64) -> Int128 {
    basic::make_int128(l, h)
}

/// `MTP_int256`.
pub const fn mtp_int256(l: Int128, h: Int128) -> Int256 {
    basic::make_int256(l, h)
}

/// `MTP_double`.
pub const fn mtp_double(v: f64) -> f64 {
    v
}

/// `MTP_flags`: flags are passed as they are.
pub const fn mtp_flags<Tag>(v: Flags<Tag>) -> Flags<Tag> {
    v
}

/// `MTP_string(const QString &)` / `MTP_string(const char *)`: UTF-8 bytes.
pub fn mtp_string(v: impl AsRef<str>) -> Vec<u8> {
    basic::make_string(v)
}

/// `MTP_bytes`.
pub fn mtp_bytes(v: impl AsRef<[u8]>) -> Vec<u8> {
    basic::make_bytes(v)
}

/// `MTP_vector<T>(...)`.
pub fn mtp_vector<T>(v: impl Into<Vec<T>>) -> Vec<T> {
    v.into()
}

/// `qs(const MTPstring &)`: decodes UTF-8 (invalid sequences → U+FFFD).
pub fn qs(v: &[u8]) -> String {
    basic::utf16(v)
}

/// `qba(const MTPstring &)`.
pub fn qba(v: &[u8]) -> Vec<u8> {
    basic::utf8(v)
}

/// `MTP_bool` (`type_utils.h`).
pub const fn mtp_bool(v: bool) -> bool {
    v
}

/// `mtpIsTrue` (`type_utils.h`).
pub const fn mtp_is_true(v: bool) -> bool {
    v
}

/// `mtpIsFalse` (`type_utils.h`).
pub const fn mtp_is_false(v: bool) -> bool {
    !v
}

/// `mtpCastFlags`: only for the constructor pairs of the scheme's
/// `flagInheritance` (e.g. `messageService` → `message`).
pub const fn mtp_cast_flags<Child, Parent>(flags: Flags<Child>) -> Flags<Parent>
where
    Child: crate::flags::FlagsInherit<Parent>,
{
    flags.inherit()
}
