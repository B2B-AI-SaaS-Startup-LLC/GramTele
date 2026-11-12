//! TL (Type Language) runtime: the Rust port of `Telegram/lib_tl/tl`.
//!
//! This crate holds everything the generated schema (`tg-scheme`) is built
//! upon:
//!
//! * [`Buffer`] / [`Reader`] — the `mtpBuffer` accumulator and the
//!   `const mtpPrime *from, *end` cursor of the original;
//! * the primitive TL types (`int`, `long`, `double`, `int128`, `int256`,
//!   `string`/`bytes`, `Vector`/`vector`, `Bool`) with byte-exact
//!   serialization;
//! * [`Flags`] — the typed flags word (`base::flags<Flag>` of the original);
//! * the [`Serialize`], [`Deserialize`], [`Boxed`], [`Identifiable`] and
//!   [`Function`] traits implemented by all generated types;
//! * [`core_types`] — the serialization-level subset of
//!   `mtproto/core_types.h` and `mtproto/type_utils.h` (`MTP_int`,
//!   `MTP_string`, `qs`, `mtpIsTrue`, ...), so that translated code has a
//!   place to call them from before the `mtproto` port lands.
//!
//! Reading never panics: every read is bounds-checked and returns
//! [`Error`] on truncated input, unknown constructors or malformed strings.
#![forbid(unsafe_code)]

pub mod core_types;
pub mod error;
pub mod flags;
pub mod tl;

pub use error::{Error, Result};
pub use flags::{Flags, FlagsInherit};
pub use tl::tl_basic_types::*;
pub use tl::tl_boxed::*;
pub use tl::tl_type_owner::TypeOwner;
