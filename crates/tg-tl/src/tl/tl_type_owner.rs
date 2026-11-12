//! Port of `lib_tl/tl/tl_type_owner.h`.
//!
//! The original keeps constructor data in an intrusively reference-counted
//! `tl::details::type_data` owned through `type_owner`, so copying an
//! `MTPUser` only bumps an atomic counter and the data is immutable after
//! construction. The port replaces this with [`std::sync::Arc`]: every
//! generated enum variant of a constructor with parameters holds
//! `Arc<XData>`. Cloning is equally cheap, the data is shared and immutable,
//! and values can be sent between threads like in the original.
//! `queryData<T>()` becomes the generated `c_x()` / `data()` getters.

/// Shared owner of constructor data (`tl::details::type_owner`).
pub type TypeOwner<T> = std::sync::Arc<T>;
