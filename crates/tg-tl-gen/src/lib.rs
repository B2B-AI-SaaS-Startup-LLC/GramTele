//! TL scheme parser and Rust code generator: the port of
//! `Telegram/lib_tl/tl/generate_tl.py` (read-write section).
//!
//! * [`generate_tl`] parses `.tl` files the way the Python script does,
//!   including the constructor id check and all scheme options;
//! * [`names`] holds the C++ → Rust naming rule;
//! * [`rust`] emits the Rust code and the metadata tables.
//!
//! `tg-scheme`'s build script calls [`generate_files`].
#![forbid(unsafe_code)]

pub mod generate_tl;
pub mod names;
pub mod rust;

pub use generate_tl::{parse, Config, Input, Schema};
pub use rust::{generate, NameMapping, Names, Output};

/// Parses `inputs` with `config` and generates the Rust sources.
pub fn generate_files(inputs: &[Input], config: &Config) -> Result<(Schema, Output), String> {
    let schema = parse(inputs, config)?;
    let output = generate(&schema)?;
    Ok((schema, output))
}
