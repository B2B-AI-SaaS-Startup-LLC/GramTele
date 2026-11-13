//! Порт: Telegram/SourceFiles/{storage,main,data,api,core,lang,countries} и корневые apiwrap/config/settings/logs/stdafx, history/history*, history_item* оригинала AyuGramDesktop.
#![forbid(unsafe_code)]
extern crate self as tg_core;
pub mod core;
pub mod countries;
pub mod lang;
#[path = "main/mod.rs"]
pub mod main;
pub mod storage;
