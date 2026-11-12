//! Port of `lib_tl/tl/tl_boxed.h`, plus the traits the generator relies on.
//!
//! In the original `tl::boxed<bare>` adds the constructor id in front of a
//! bare value on write and reads it before dispatching on read. In the port
//! the boxed / bare distinction is carried by the Rust type itself (see
//! [`crate::Serialize`]): generated enums, `Vec<T>` and `bool` are always
//! written boxed, constructor data structs and primitives bare. The bare
//! read with an already known id (`bare::read(from, end, cons)`) is
//! [`Boxed::read_bare`].

use crate::error::Result;
use crate::tl::tl_basic_types::{
    read_bool_bare, read_vector_bare, Deserialize, Reader, Serialize, TypeId, ID_BOOL_FALSE,
    ID_BOOL_TRUE, ID_VECTOR,
};

/// A value written with a leading constructor id (`tl::boxed<T>`).
pub trait Boxed: Serialize + Deserialize {
    /// The constructor id of this value (`type()`).
    fn type_id(&self) -> TypeId;
    /// Reads the body of the value whose constructor id `cons` was already
    /// consumed (`bare::read(from, end, cons)`).
    fn read_bare(from: &mut Reader<'_>, cons: TypeId) -> Result<Self>;
}

/// A definition with a fixed constructor id (`mtpc_x`): constructor data
/// structs and method structs.
pub trait Identifiable {
    /// The constructor id.
    const ID: TypeId;
}

/// An RPC method (`---functions---`): serializes boxed (id + parameters)
/// and names its result type (`ResponseType`).
pub trait Function: Serialize {
    /// Type of the result (`using ResponseType = ...`).
    type Response: Deserialize;
}

impl<T: Serialize + Deserialize> Boxed for Vec<T> {
    fn type_id(&self) -> TypeId {
        ID_VECTOR
    }
    fn read_bare(from: &mut Reader<'_>, cons: TypeId) -> Result<Self> {
        read_vector_bare(from, cons)
    }
}

impl Boxed for bool {
    fn type_id(&self) -> TypeId {
        if *self {
            ID_BOOL_TRUE
        } else {
            ID_BOOL_FALSE
        }
    }
    fn read_bare(_from: &mut Reader<'_>, cons: TypeId) -> Result<Self> {
        read_bool_bare(cons)
    }
}
