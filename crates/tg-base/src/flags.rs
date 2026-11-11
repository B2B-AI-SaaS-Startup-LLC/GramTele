//! Port of `base/flags.h`: type-safe bit flags over an enum.
//!
//! In C++ any scoped enum with `is_flag_type(Enum)` found by ADL gets the
//! operators `| & ^ ~ !` and `base::flags<Enum>` is the combined value. Here
//! a fieldless enum is declared a flag type with [`flag_type!`], after which
//! `A | B`, `flags & Flag::X`, `!flags::empty()` etc. work the same way:
//!
//! ```
//! use tg_base::flags::Flags;
//!
//! #[derive(Clone, Copy, PartialEq, Eq, Debug)]
//! #[repr(u32)]
//! enum Flag {
//!     One = 1 << 0,
//!     Two = 1 << 1,
//! }
//! tg_base::flag_type!(Flag: u32);
//!
//! let both: Flags<Flag> = Flag::One | Flag::Two;
//! assert!((both & Flag::Two).any()); // `if (flags & Flag::Two)`
//! assert_eq!(both & !Flags::from(Flag::One), Flag::Two);
//! ```
//!
//! Differences from C++ that the translation must account for:
//! - there is no implicit conversion to the integer or to `bool`: use
//!   [`Flags::value`], [`Flags::any`], [`Flags::is_empty`];
//! - `flags = 0`, `flags = nullptr`, `flags == 0` and `Flag | 0` become
//!   [`Flags::empty`], `Flags::default()` and `is_empty()`;
//! - `base::flags<Enum>::from_raw(x)` is [`Flags::from_raw`].

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

/// Integer types that can back a flags enum (`std::underlying_type_t`).
pub trait FlagRepr:
    Copy
    + Default
    + Eq
    + Ord
    + Hash
    + fmt::Debug
    + BitOr<Output = Self>
    + BitAnd<Output = Self>
    + BitXor<Output = Self>
    + Not<Output = Self>
{
}

macro_rules! impl_flag_repr {
    ($($ty:ty),*) => { $(impl FlagRepr for $ty {})* };
}
impl_flag_repr!(i8, u8, i16, u16, i32, u32, i64, u64, isize, usize);

/// A fieldless enum usable as a flag (the `is_flag_type` marker).
///
/// Implement it with [`flag_type!`], which also adds the operators.
pub trait FlagType: Copy {
    /// The underlying integer type.
    type Repr: FlagRepr;
    /// The numeric value of the flag.
    fn to_repr(self) -> Self::Repr;
}

/// Combined value of flags of one enum (`base::flags<Enum>`).
pub struct Flags<E: FlagType> {
    value: E::Repr,
    _marker: PhantomData<fn() -> E>,
}

impl<E: FlagType> Flags<E> {
    /// No flags set (`flags = 0`).
    pub fn empty() -> Self {
        Self::from_raw(E::Repr::default())
    }

    /// `base::flags<Enum>::from_raw`.
    pub fn from_raw(value: E::Repr) -> Self {
        Self {
            value,
            _marker: PhantomData,
        }
    }

    /// The raw integer (`flags.value()` or the implicit conversion).
    pub fn value(self) -> E::Repr {
        self.value
    }

    /// True if any bit is set (`if (flags)`).
    pub fn any(self) -> bool {
        self.value != E::Repr::default()
    }

    /// True if no bit is set (`!flags`, `flags == 0`).
    pub fn is_empty(self) -> bool {
        !self.any()
    }
}

impl<E: FlagType> Clone for Flags<E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E: FlagType> Copy for Flags<E> {}

impl<E: FlagType> Default for Flags<E> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<E: FlagType> fmt::Debug for Flags<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Flags({:?})", self.value)
    }
}

impl<E: FlagType> PartialEq for Flags<E> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<E: FlagType> Eq for Flags<E> {}

impl<E: FlagType> Hash for Flags<E> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<E: FlagType> PartialOrd for Flags<E> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<E: FlagType> Ord for Flags<E> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl<E: FlagType> From<E> for Flags<E> {
    fn from(value: E) -> Self {
        Self::from_raw(value.to_repr())
    }
}

impl<E: FlagType> PartialEq<E> for Flags<E> {
    fn eq(&self, other: &E) -> bool {
        self.value == other.to_repr()
    }
}

impl<E: FlagType> PartialOrd<E> for Flags<E> {
    fn partial_cmp(&self, other: &E) -> Option<Ordering> {
        Some(self.value.cmp(&other.to_repr()))
    }
}

impl<E: FlagType> Not for Flags<E> {
    type Output = Self;
    fn not(self) -> Self {
        Self::from_raw(!self.value)
    }
}

macro_rules! impl_flags_binary_ops {
    ($($trait:ident $method:ident $assign_trait:ident $assign_method:ident $op:tt;)*) => {$(
        impl<E: FlagType> $trait for Flags<E> {
            type Output = Self;
            fn $method(self, rhs: Self) -> Self {
                Self::from_raw(self.value $op rhs.value)
            }
        }
        impl<E: FlagType> $trait<E> for Flags<E> {
            type Output = Self;
            fn $method(self, rhs: E) -> Self {
                Self::from_raw(self.value $op rhs.to_repr())
            }
        }
        impl<E: FlagType> $assign_trait for Flags<E> {
            fn $assign_method(&mut self, rhs: Self) {
                self.value = self.value $op rhs.value;
            }
        }
        impl<E: FlagType> $assign_trait<E> for Flags<E> {
            fn $assign_method(&mut self, rhs: E) {
                self.value = self.value $op rhs.to_repr();
            }
        }
    )*};
}

impl_flags_binary_ops! {
    BitOr bitor BitOrAssign bitor_assign |;
    BitAnd bitand BitAndAssign bitand_assign &;
    BitXor bitxor BitXorAssign bitxor_assign ^;
}

/// Declares a fieldless enum as a flag type (`is_flag_type`).
///
/// `flag_type!(Flag: u32)` implements [`FlagType`] with the given
/// underlying type (it must match the enum's `#[repr]`) and the operators
/// `Flag | Flag`, `Flag | Flags<Flag>`, `Flag & Flags<Flag>`, `!Flag` and
/// friends, whose results are `Flags<Flag>`.
#[macro_export]
macro_rules! flag_type {
    ($enum:ty : $repr:ty) => {
        impl $crate::flags::FlagType for $enum {
            type Repr = $repr;
            fn to_repr(self) -> $repr {
                self as $repr
            }
        }
        impl ::core::ops::Not for $enum {
            type Output = $crate::flags::Flags<$enum>;
            fn not(self) -> Self::Output {
                !$crate::flags::Flags::from(self)
            }
        }
        impl ::core::cmp::PartialEq<$crate::flags::Flags<$enum>> for $enum {
            fn eq(&self, other: &$crate::flags::Flags<$enum>) -> bool {
                *other == *self
            }
        }
        $crate::flag_type!(@binary $enum, BitOr bitor);
        $crate::flag_type!(@binary $enum, BitAnd bitand);
        $crate::flag_type!(@binary $enum, BitXor bitxor);
    };
    (@binary $enum:ty, $trait:ident $method:ident) => {
        impl ::core::ops::$trait for $enum {
            type Output = $crate::flags::Flags<$enum>;
            fn $method(self, rhs: $enum) -> Self::Output {
                ::core::ops::$trait::$method($crate::flags::Flags::from(self), rhs)
            }
        }
        impl ::core::ops::$trait<$crate::flags::Flags<$enum>> for $enum {
            type Output = $crate::flags::Flags<$enum>;
            fn $method(self, rhs: $crate::flags::Flags<$enum>) -> Self::Output {
                ::core::ops::$trait::$method($crate::flags::Flags::from(self), rhs)
            }
        }
    };
}

/// Declares `Ext` as an extended flag set of `Base` (`extended_flags<Ext>`):
/// a second enum whose values are valid flags of `Base` and can be mixed
/// with it. `extended_flags!(Ext => Base)`; `Base` must be a [`FlagType`].
///
/// Operators with an `Ext` operand give `Flags<Base>`.
#[macro_export]
macro_rules! extended_flags {
    ($ext:ty => $base:ty) => {
        impl $ext {
            /// `extended_flag_convert`: the value as a flag of the base enum.
            #[allow(dead_code)]
            pub fn to_base_flags(self) -> $crate::flags::Flags<$base> {
                $crate::flags::Flags::from_raw(self as <$base as $crate::flags::FlagType>::Repr)
            }
        }
        impl ::core::ops::Not for $ext {
            type Output = $crate::flags::Flags<$base>;
            fn not(self) -> Self::Output {
                !self.to_base_flags()
            }
        }
        impl ::core::cmp::PartialEq<$ext> for $crate::flags::Flags<$base> {
            fn eq(&self, other: &$ext) -> bool {
                *self == other.to_base_flags()
            }
        }
        impl ::core::cmp::PartialEq<$crate::flags::Flags<$base>> for $ext {
            fn eq(&self, other: &$crate::flags::Flags<$base>) -> bool {
                *other == self.to_base_flags()
            }
        }
        $crate::extended_flags!(@binary $ext, $base, BitOr bitor BitOrAssign bitor_assign);
        $crate::extended_flags!(@binary $ext, $base, BitAnd bitand BitAndAssign bitand_assign);
        $crate::extended_flags!(@binary $ext, $base, BitXor bitxor BitXorAssign bitxor_assign);
    };
    (@binary $ext:ty, $base:ty, $trait:ident $method:ident $assign_trait:ident $assign_method:ident) => {
        impl ::core::ops::$trait<$ext> for $crate::flags::Flags<$base> {
            type Output = $crate::flags::Flags<$base>;
            fn $method(self, rhs: $ext) -> Self::Output {
                ::core::ops::$trait::$method(self, rhs.to_base_flags())
            }
        }
        impl ::core::ops::$assign_trait<$ext> for $crate::flags::Flags<$base> {
            fn $assign_method(&mut self, rhs: $ext) {
                ::core::ops::$assign_trait::$assign_method(self, rhs.to_base_flags());
            }
        }
        impl ::core::ops::$trait<$crate::flags::Flags<$base>> for $ext {
            type Output = $crate::flags::Flags<$base>;
            fn $method(self, rhs: $crate::flags::Flags<$base>) -> Self::Output {
                ::core::ops::$trait::$method(self.to_base_flags(), rhs)
            }
        }
        impl ::core::ops::$trait<$ext> for $ext {
            type Output = $crate::flags::Flags<$base>;
            fn $method(self, rhs: $ext) -> Self::Output {
                ::core::ops::$trait::$method(self.to_base_flags(), rhs.to_base_flags())
            }
        }
        impl ::core::ops::$trait<$base> for $ext {
            type Output = $crate::flags::Flags<$base>;
            fn $method(self, rhs: $base) -> Self::Output {
                ::core::ops::$trait::$method(self.to_base_flags(), rhs)
            }
        }
        impl ::core::ops::$trait<$ext> for $base {
            type Output = $crate::flags::Flags<$base>;
            fn $method(self, rhs: $ext) -> Self::Output {
                ::core::ops::$trait::$method(rhs.to_base_flags(), self)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    mod flags_namespace {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        #[repr(i32)]
        pub enum Flag {
            One = 1 << 0,
            Two = 1 << 1,
            Three = 1 << 2,
        }
        crate::flag_type!(Flag: i32);

        pub mod class {
            #[derive(Clone, Copy, PartialEq, Eq, Debug)]
            #[repr(i64)]
            pub enum Public {
                One = 1 << 2,
                Two = 1 << 1,
                Three = 1 << 0,
            }
            crate::flag_type!(Public: i64);

            #[derive(Clone, Copy, PartialEq, Eq, Debug)]
            #[repr(i64)]
            enum Private {
                One = 1 << 0,
                Two = 1 << 1,
                Three = 1 << 2,
            }
            crate::flag_type!(Private: i64);

            pub fn test_private() {
                super::super::test_flags!(Private::One, Private::Two, Private::Three);
            }
        }
    }

    mod extended_namespace {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        #[repr(i32)]
        pub enum Flag {
            One = 1 << 3,
            Two = 1 << 4,
            Three = 1 << 5,
        }
        crate::extended_flags!(Flag => super::flags_namespace::Flag);
    }

    // The body of `MethodNamespace::TestFlags<Enum>`; a macro because the
    // extended enum produces `Flags<Base>` from `a | b`, which a single
    // generic function could not express.
    macro_rules! test_flags {
        ($a:expr, $b:expr, $c:expr) => {{
            let (a, b, c) = ($a, $b, $c);
            let mut abc = a | b;
            abc |= c;
            assert!(abc != a);
            assert!(abc != (a | b));
            assert!((abc & a) == a);
            assert!((abc & b) == b);
            assert!((abc & c) == c);
            assert!((abc & !a) == (b | c));
            assert!((abc & !(b | c)) == a);
            assert!((abc ^ a) == (abc & !a));

            let mut another = a | b;
            another |= c;
            assert!(abc == another);
            another &= !b;
            assert!(another == (a | c));
            another ^= a;
            assert!(another == c);

            let mut another = a & !a;
            assert!(another.is_empty());
            another = (b ^ b);
            let is_zero = (another & abc).value() == 0;
            assert!(is_zero);
            assert!(!(another & abc).any());
            let more = (a | another).value();
            let just = (a | a).value();
            assert_eq!(more, just);
            assert!((a | a).any());
        }};
    }
    pub(crate) use test_flags;

    #[test]
    fn operators_on_scoped_enums_non_member() {
        use flags_namespace::Flag;
        test_flags!(Flag::One, Flag::Two, Flag::Three);
    }

    #[test]
    fn operators_on_public_member_enums() {
        use flags_namespace::class::Public;
        test_flags!(Public::One, Public::Two, Public::Three);
    }

    #[test]
    fn operators_on_private_member_enums() {
        flags_namespace::class::test_private();
    }

    #[test]
    fn operators_on_extended_flags() {
        use extended_namespace::Flag as Ext;
        use flags_namespace::Flag as Base;
        test_flags!(Ext::One, Ext::Two, Ext::Three);
        let onetwo = Base::One | Ext::Two;
        let twoone = Ext::Two | Base::One;
        assert!(onetwo == twoone);
        assert_eq!(onetwo.value(), 1 | (1 << 4));
    }

    #[test]
    fn extended_flags_mixed_with_flags_value() {
        use extended_namespace::Flag as Ext;
        use flags_namespace::Flag as Base;
        let mut value = Flags::from(Base::One);
        value |= Ext::Three;
        assert_eq!(value.value(), 1 | (1 << 5));
        value &= !Ext::Three;
        assert!(value == Base::One);
        value ^= Ext::Two;
        assert_eq!((value & Ext::Two).value(), 1 << 4);
        assert!(Ext::Two == (Ext::Two | Ext::Two));
    }

    #[test]
    fn comparisons_are_numeric() {
        use flags_namespace::Flag;
        let one_two = Flag::One | Flag::Two;
        assert!(one_two > Flag::Two);
        assert!(one_two < Flag::Three);
        assert!(one_two <= Flag::Three);
        assert!(Flags::from(Flag::One) < Flag::Two);
        assert!(Flags::from(Flag::One) <= Flag::One);
        assert!(Flag::One == Flags::from(Flag::One));
    }

    #[test]
    fn not_inverts_all_bits_of_signed_repr() {
        use flags_namespace::Flag;
        let inverted = !Flag::One;
        assert_eq!(inverted.value(), !1i32);
        assert!((inverted & Flag::One).is_empty());
        assert!((inverted & Flag::Two).any());
    }

    #[test]
    fn unsigned_repr_wraps_like_cpp() {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        #[repr(u8)]
        enum Small {
            Top = 0x80,
        }
        crate::flag_type!(Small: u8);
        assert_eq!((!Small::Top).value(), 0x7f);
        assert_eq!((Small::Top | Small::Top).value(), 0x80);
    }

    #[test]
    fn flags_are_hashable_and_ordered() {
        use flags_namespace::Flag;
        let mut set = std::collections::BTreeSet::new();
        set.insert(Flag::Two | Flag::One);
        set.insert(Flags::from(Flag::Three));
        set.insert(Flag::One | Flag::Two);
        let values: Vec<_> = set.iter().map(|f| f.value()).collect();
        assert_eq!(values, vec![3, 4]);
    }
}
