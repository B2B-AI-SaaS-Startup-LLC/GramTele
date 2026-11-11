//! Port of `base/enum_mask.h`: a set of up to 32 enum values.

use std::marker::PhantomData;

/// An enum usable in [`EnumMask`]; `COUNT` is the `Enum::kCount` of the
/// original and must not exceed 32.
pub trait EnumMaskValue: Copy {
    const COUNT: u32;
    fn index(self) -> u32;
}

/// `base::enum_mask<Enum>`.
pub struct EnumMask<E: EnumMaskValue> {
    value: u32,
    _marker: PhantomData<fn() -> E>,
}

impl<E: EnumMaskValue> EnumMask<E> {
    const CHECK: () = assert!(E::COUNT <= 32, "We have only 32 bit.");

    fn to_bit(value: E) -> u32 {
        let () = Self::CHECK;
        1u32 << value.index()
    }

    /// Empty mask.
    pub fn new() -> Self {
        let () = Self::CHECK;
        Self {
            value: 0,
            _marker: PhantomData,
        }
    }

    /// All 32 bits set (like the original, not limited to `COUNT`).
    pub fn all() -> Self {
        Self {
            value: !0,
            ..Self::new()
        }
    }

    #[must_use]
    pub fn added(self, other: Self) -> Self {
        Self {
            value: self.value | other.value,
            ..self
        }
    }

    pub fn set(&mut self, other: Self) {
        self.value |= other.value;
    }

    pub fn test(&self, value: E) -> bool {
        self.value & Self::to_bit(value) != 0
    }

    /// `explicit operator bool`.
    pub fn any(&self) -> bool {
        self.value != 0
    }
}

impl<E: EnumMaskValue> From<E> for EnumMask<E> {
    fn from(value: E) -> Self {
        Self {
            value: Self::to_bit(value),
            ..Self::new()
        }
    }
}

impl<E: EnumMaskValue> Default for EnumMask<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: EnumMaskValue> Clone for EnumMask<E> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<E: EnumMaskValue> Copy for EnumMask<E> {}

impl<E: EnumMaskValue> std::fmt::Debug for EnumMask<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EnumMask({:#b})", self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    enum Kind {
        A,
        B,
        C,
        Last,
    }
    impl EnumMaskValue for Kind {
        const COUNT: u32 = Kind::Last as u32;
        fn index(self) -> u32 {
            self as u32
        }
    }

    #[test]
    fn set_and_test() {
        let mut mask = EnumMask::<Kind>::new();
        assert!(!mask.any());
        mask.set(Kind::B.into());
        assert!(mask.any());
        assert!(mask.test(Kind::B));
        assert!(!mask.test(Kind::A));
        let more = mask.added(Kind::C.into());
        assert!(more.test(Kind::C) && more.test(Kind::B));
        assert!(!mask.test(Kind::C));
        assert!(EnumMask::<Kind>::all().test(Kind::A));
        assert!(EnumMask::from(Kind::A).test(Kind::A));
    }
}
