//! Порт: Telegram/lib_base оригинала AyuGramDesktop.

#![forbid(unsafe_code)]

pub mod algorithm;
pub mod assertion;
pub mod base_file_utilities;
pub mod basic_types;
pub mod binary_guard;
pub mod bytes;
pub mod crc32hash;
pub mod debug_log;
pub mod enum_mask;
pub mod file_lock;
pub mod flags;
pub mod flat_map;
pub mod flat_set;
pub mod index_based_iterator;
pub mod last_used_cache;
pub mod never_freed_pointer;
pub mod openssl_help;
pub mod parse_helper;
pub mod qthelp_regex;
pub mod qthelp_url;
pub mod random;
pub mod runtime_composer;
pub mod thread_safe_wrap;
pub mod unixtime;
pub mod weak_ptr;
pub mod zlib_help;
