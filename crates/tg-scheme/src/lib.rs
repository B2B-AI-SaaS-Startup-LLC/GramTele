//! The generated Telegram API + MTProto scheme (`api.tl`, `mtproto.tl`).
//!
//! The port of the generated `scheme.h` / `scheme.cpp`. Names follow a
//! fixed rule from the C++ names (see `README.md` of this crate):
//! `MTPInputPeer` → [`InputPeer`], `MTPDuser` → [`UserData`],
//! `mtpc_user` → [`UserData::ID`], `MTP_inputPeerUser(...)` →
//! [`mtp_input_peer_user`]`(...)`, `MTPmessages_GetHistory(...)` →
//! [`MessagesGetHistory::new`]`(...)`.
#![forbid(unsafe_code)]
#![allow(
    clippy::too_many_arguments,
    clippy::upper_case_acronyms,
    clippy::new_without_default
)]

pub mod dump_to_text;
pub mod meta;

pub use tg_tl as tl;

use std::sync::Arc;

include!(concat!(env!("OUT_DIR"), "/scheme.rs"));
