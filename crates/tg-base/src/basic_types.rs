//! Port of `base/basic_types.h`.
//!
//! Only the project-specific aliases need code; the rest is replaced:
//! `int8..uint64` -> `i8..u64`, `float32/64` -> `f32/f64`,
//! `not_null<T*>` -> `&T`/`&mut T` (or `NonNull`-free ids into a store),
//! `gsl::index`/`index_type`/`size_type` -> `isize`/`usize`,
//! `Fn<R(A)>` -> `Rc<dyn Fn(A) -> R>` (`Arc<dyn Fn + Send + Sync>` across
//! threads), `FnMut<R(A)>` (`unique_function`) -> `Box<dyn FnMut(A) -> R>`,
//! a nullable one -> `Option<Box<dyn FnMut(A) -> R>>`,
//! `"text"_q` -> `"text"`, `base::required<T>` -> a plain field.

/// Seconds since the Unix epoch as used by the TL layer.
pub type TimeId = i32;
