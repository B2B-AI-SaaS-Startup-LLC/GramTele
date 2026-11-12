//! Typed TL flags word.
//!
//! In the original every constructor or method with a `flags:#` parameter
//! gets a nested `enum class Flag` and `using Flags = base::flags<Flag>`,
//! wrapped into `MTPflags<...>` for serialization. When a definition also has
//! `flags2:#`, the original merges both words into one 64-bit value whose
//! bits 32..63 are the `flags2` bits (`f_x = 1ULL << (32 + n)`).
//!
//! The port uses one generic type, `Flags<Tag>`, where `Tag` is the generated
//! data struct or method struct. The tag keeps flags of different
//! definitions apart at compile time (like the distinct `Flag` enums), while
//! all bit operations are implemented once here. The raw value is always
//! 64 bits wide; the generated `write` emits one prime for definitions with a
//! single `flags` word and two primes (low, high) for `flags` + `flags2`,
//! exactly like `tl::flags_type::write`.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

/// Marker: the flags of `Self` are a subset of the flags of `Parent` with the
/// same bits (`flagInheritance`), so [`Flags::inherit`] may convert them.
pub trait FlagsInherit<Parent> {}

/// The flags word of the definition `Tag` (`MTPDx::Flags` / `MTPx::Flags`).
pub struct Flags<Tag> {
    raw: u64,
    tag: PhantomData<fn(&Tag)>,
}

impl<Tag> Flags<Tag> {
    /// No bits set (`MTP_flags(0)`, `Flags(0)`).
    pub const fn empty() -> Self {
        Self::from_raw(0)
    }

    /// Wraps a raw value (`Flags::from_raw`).
    pub const fn from_raw(raw: u64) -> Self {
        Self {
            raw,
            tag: PhantomData,
        }
    }

    /// The raw value (`flags.value()`, `MTPflags::v.value()`).
    pub const fn value(self) -> u64 {
        self.raw
    }

    /// `true` if no bit is set.
    pub const fn is_empty(self) -> bool {
        self.raw == 0
    }

    /// `true` if all bits of `other` are set.
    pub const fn contains(self, other: Self) -> bool {
        (self.raw & other.raw) == other.raw
    }

    /// `true` if any bit of `other` is set (`if (flags & Flag::f_x)`).
    pub const fn intersects(self, other: Self) -> bool {
        (self.raw & other.raw) != 0
    }

    /// `true` if bit number `bit` (0..63) is set. Used by generated code.
    pub const fn bit(self, bit: u32) -> bool {
        (self.raw >> bit) & 1 != 0
    }

    /// Sets the bits of `other`.
    pub fn insert(&mut self, other: Self) {
        self.raw |= other.raw;
    }

    /// Clears the bits of `other`.
    pub fn remove(&mut self, other: Self) {
        self.raw &= !other.raw;
    }

    /// Sets or clears the bits of `other`.
    pub fn set(&mut self, other: Self, value: bool) {
        if value {
            self.insert(other);
        } else {
            self.remove(other);
        }
    }

    /// Returns `other` if `condition` holds and no bits otherwise
    /// (the `(condition ? Flag::f_x : Flag(0))` idiom).
    pub const fn when(condition: bool, other: Self) -> Self {
        if condition {
            other
        } else {
            Self::empty()
        }
    }

    /// Reinterprets the bits as flags of another definition.
    ///
    /// The generated `From` impls (the port of `mtpCastFlags`) use this for
    /// the definition pairs listed in the scheme's `flagInheritance`; call it
    /// directly only when the bit layouts are known to be compatible.
    pub const fn cast<Other>(self) -> Flags<Other> {
        Flags::from_raw(self.raw)
    }

    /// `mtpCastFlags`: converts the flags of a constructor into the flags of
    /// its "flags parent" (`flagInheritance` in the scheme options). Only
    /// the pairs the generator declared with [`FlagsInherit`] compile.
    pub const fn inherit<Parent>(self) -> Flags<Parent>
    where
        Tag: FlagsInherit<Parent>,
    {
        Flags::from_raw(self.raw)
    }
}

impl<Tag> Clone for Flags<Tag> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Tag> Copy for Flags<Tag> {}

impl<Tag> Default for Flags<Tag> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<Tag> PartialEq for Flags<Tag> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<Tag> Eq for Flags<Tag> {}

impl<Tag> Hash for Flags<Tag> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<Tag> fmt::Debug for Flags<Tag> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Flags(0x{:x})", self.raw)
    }
}

impl<Tag> BitOr for Flags<Tag> {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self::from_raw(self.raw | rhs.raw)
    }
}

impl<Tag> BitAnd for Flags<Tag> {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self::from_raw(self.raw & rhs.raw)
    }
}

impl<Tag> BitXor for Flags<Tag> {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self {
        Self::from_raw(self.raw ^ rhs.raw)
    }
}

impl<Tag> Not for Flags<Tag> {
    type Output = Self;
    /// Complements all 64 bits. For a single-word definition only the low 32
    /// bits are ever written, so the serialized result equals the original's
    /// 32-bit `~flags`; `value()` however differs in the high half.
    fn not(self) -> Self {
        Self::from_raw(!self.raw)
    }
}

impl<Tag> BitOrAssign for Flags<Tag> {
    fn bitor_assign(&mut self, rhs: Self) {
        self.raw |= rhs.raw;
    }
}

impl<Tag> BitAndAssign for Flags<Tag> {
    fn bitand_assign(&mut self, rhs: Self) {
        self.raw &= rhs.raw;
    }
}

impl<Tag> BitXorAssign for Flags<Tag> {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.raw ^= rhs.raw;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct A;
    struct B;

    #[test]
    fn bit_operations() {
        const X: Flags<A> = Flags::from_raw(1 << 3);
        const Y: Flags<A> = Flags::from_raw(1 << 35);
        let mut f = X | Y;
        assert_eq!(f.value(), (1 << 3) | (1 << 35));
        assert!(f.contains(X) && f.contains(Y) && f.contains(X | Y));
        assert!(f.bit(3) && f.bit(35) && !f.bit(4));
        f.remove(X);
        assert!(!f.intersects(X));
        f.set(X, true);
        assert!(f.intersects(X));
        assert_eq!((f & X), X);
        assert_eq!(Flags::<A>::when(false, X), Flags::empty());
        let g: Flags<B> = f.cast();
        assert_eq!(g.value(), f.value());
        assert!(Flags::<A>::default().is_empty());
    }
}
