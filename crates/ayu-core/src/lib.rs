#![forbid(unsafe_code)]
//! Данные AyuGram. Пользовательские файлы открываются только по явному пути.
pub mod ayu_settings;
pub mod ayu_state;
pub mod data;

pub mod libs;
use libs::json_ext;
