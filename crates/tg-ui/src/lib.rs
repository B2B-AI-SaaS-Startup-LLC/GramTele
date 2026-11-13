//! Порт: Telegram/lib_ui оригинала AyuGramDesktop.
//!
//! Only the text value model is here so far (`ui/text/text_entity.h`,
//! `ui/text/text_utilities`): the core needs it before the window layer
//! exists. Link and markdown parsing, painting and widgets are stage 5.
#![forbid(unsafe_code)]
pub mod effects;
pub mod text;
