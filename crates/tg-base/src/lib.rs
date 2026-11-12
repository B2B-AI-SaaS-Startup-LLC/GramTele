//! Порт: Telegram/lib_base оригинала AyuGramDesktop.

#![forbid(unsafe_code)]

pub mod algorithm;
pub mod assertion;
pub mod base_file_utilities;
pub mod basic_types;
pub mod battery_saving;
pub mod binary_guard;
pub mod bytes;
pub mod call_delayed;
pub mod concurrent_timer;
pub mod crash_report_header;
pub mod crc32hash;
pub mod custom_app_icon;
pub mod debug_destroy_informer;
pub mod debug_log;
pub mod enum_mask;
pub mod file_lock;
pub mod flags;
pub mod flat_map;
pub mod flat_set;
pub mod index_based_iterator;
pub mod integration;
pub mod invoke_queued;
pub mod last_used_cache;
pub mod last_user_input;
pub mod network_reachability;
pub mod never_freed_pointer;
pub mod openssl_help;
pub mod options;
pub mod parse_helper;
pub mod platform;
pub mod power_save_blocker;
pub mod qthelp_regex;
pub mod qthelp_url;
pub mod random;
pub mod runtime_composer;
pub mod screen_reader_state;
pub mod single_instance;
pub mod system_unlock;
pub mod thread_safe_wrap;
pub mod timer;
pub mod timer_rpl;
pub mod unixtime;
pub mod weak_ptr;
pub mod zlib_help;
