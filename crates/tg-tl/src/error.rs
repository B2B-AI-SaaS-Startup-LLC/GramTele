//! Read errors.
//!
//! The original `read()` methods return a plain `bool`; the port returns a
//! [`Result`] so the caller can tell why decoding failed. Every case where the
//! original returns `false` maps to one of these variants.

use std::fmt;

/// Why a TL value could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Error {
    /// The buffer ended before the value was complete (also covers a vector
    /// whose declared length exceeds the remaining primes, which the original
    /// rejects with the same `Has(count)` check).
    UnexpectedEnd,
    /// The constructor id is not one of the constructors of the expected type.
    UnexpectedConstructor {
        /// The id found in the buffer.
        id: u32,
    },
    /// A string whose first byte is 255 (neither short nor long form).
    InvalidString,
}

/// Result of reading a TL value.
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnexpectedEnd => f.write_str("unexpected end of TL buffer"),
            Error::UnexpectedConstructor { id } => {
                write!(f, "unexpected TL constructor 0x{id:08x}")
            }
            Error::InvalidString => f.write_str("invalid TL string length prefix"),
        }
    }
}

impl std::error::Error for Error {}
