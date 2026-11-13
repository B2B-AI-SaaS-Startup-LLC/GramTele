//! `Storage::Cache`: encrypted media cache, independent of Main::Session.
pub mod storage_cache_binlog_reader;
mod storage_cache_cleaner;
mod storage_cache_compactor;
pub mod storage_cache_database;
mod storage_cache_database_object;
pub mod storage_cache_types;
pub use storage_cache_database::{Database, Environment};
pub use storage_cache_types::{
    Error, ErrorType, Key, Settings, SettingsUpdate, Stats, TaggedSummary, TaggedValue,
};
