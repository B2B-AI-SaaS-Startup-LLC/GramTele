//! Scheme metadata: every constructor and method with its parameters, as
//! the generator saw it. Used by [`crate::dump_to_text`] (the port of the
//! generated `scheme-dump_to_text.cpp`) and by the tests, which build sample
//! buffers from it independently of the generated readers and writers.

/// A parameter or result type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    /// `int`
    Int,
    /// `long`
    Long,
    /// `double`
    Double,
    /// `string`
    String,
    /// `bytes`
    Bytes,
    /// `int128`
    Int128,
    /// `int256`
    Int256,
    /// A boxed type, index into [`TYPES`].
    Boxed(u16),
    /// A constructor used bare, index into [`CONSTRUCTORS`].
    Bare(u16),
    /// `Vector<T>` (boxed) or `vector<T>`.
    Vector {
        /// Written with the vector constructor id.
        boxed: bool,
        /// Item type.
        item: &'static Ty,
    },
    /// The `!X` template parameter / `X` result.
    Template,
}

/// What a parameter is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `#` (with `wide`, merged with `flags2` into two words).
    Flags {
        /// Two words.
        wide: bool,
    },
    /// `flags.N?true` (bits of `flags2` are 32..63).
    True {
        /// Bit.
        bit: u8,
    },
    /// A serialized value, conditional on `bit`.
    Value {
        /// Type.
        ty: Ty,
        /// Flag bit, if conditional.
        bit: Option<u8>,
    },
}

/// A parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Param {
    /// Name as in the scheme.
    pub name: &'static str,
    /// Kind.
    pub kind: Kind,
}

/// A constructor or a method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Def {
    /// Scheme name after renames (`messages.getHistory`).
    pub name: &'static str,
    /// C++ suffix (`messages_getHistory`), the name the original prints in
    /// text dumps.
    pub cpp_name: &'static str,
    /// Constructor id.
    pub id: u32,
    /// Parameters (`flags2` merged into the flags parameter).
    pub params: &'static [Param],
    /// Result type.
    pub result: Ty,
    /// Name of the merged `flags2` parameter.
    pub flags2: Option<&'static str>,
}

/// A boxed type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type {
    /// Scheme name.
    pub name: &'static str,
    /// Constructors, indices into [`CONSTRUCTORS`].
    pub constructors: &'static [u16],
}

include!(concat!(env!("OUT_DIR"), "/scheme_meta.rs"));

/// Finds a constructor or a method by id.
pub fn find(id: u32) -> Option<&'static Def> {
    CONSTRUCTORS.iter().chain(FUNCTIONS).find(|d| d.id == id)
}
