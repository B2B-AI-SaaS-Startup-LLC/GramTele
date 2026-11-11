//! `crl::toggle_fp_exceptions`. The original only acts on Windows, and Rust
//! floating point never traps, so this is a documented no-op kept so that
//! call sites translate one to one.

pub fn toggle_fp_exceptions(_throwing: bool) {}
