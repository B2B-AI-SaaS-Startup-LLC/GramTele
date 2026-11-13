//! Port of `core/credits_amount.h`.
//!
//! `CreditsAmountFromTL`, `StarsAmountToTL` and `PrepareCreditsAmountText`
//! are declared in that header but defined in `data/components/credits.cpp`;
//! they come with that file.
use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use tg_base::algorithm::safe_round;

/// `kOneStarInNano`.
pub const ONE_STAR_IN_NANO: i64 = 1_000_000_000;

/// `CreditsType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CreditsType {
    #[default]
    Stars,
    Ton,
}

/// `CreditsAmount`: whole units plus nano units in `0..ONE_STAR_IN_NANO`.
///
/// The original packs `_ton` and `_whole` into one 64-bit word (2 and 62
/// bits); the port keeps full fields, values that need more than 62 bits are
/// not expected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CreditsAmount {
    ton: bool,
    whole: i64,
    nano: i64,
}

impl CreditsAmount {
    /// `CreditsAmount(int64 whole, CreditsType type)`.
    pub fn new(whole: i64, type_: CreditsType) -> Self {
        Self {
            ton: type_ == CreditsType::Ton,
            whole,
            nano: 0,
        }
    }

    /// `CreditsAmount(int64 whole, int64 nano, CreditsType type)`.
    pub fn with_nano(whole: i64, nano: i64, type_: CreditsType) -> Self {
        let mut result = Self {
            ton: type_ == CreditsType::Ton,
            whole,
            nano,
        };
        result.normalize();
        result
    }

    pub fn whole(&self) -> i64 {
        self.whole
    }

    pub fn nano(&self) -> i64 {
        self.nano
    }

    pub fn value(&self) -> f64 {
        self.whole as f64 + self.nano as f64 / ONE_STAR_IN_NANO as f64
    }

    pub fn ton(&self) -> bool {
        self.ton
    }

    pub fn stars(&self) -> bool {
        !self.ton
    }

    pub fn type_(&self) -> CreditsType {
        if self.ton {
            CreditsType::Ton
        } else {
            CreditsType::Stars
        }
    }

    /// Also `operator!` (negated) and `explicit operator bool`.
    pub fn empty(&self) -> bool {
        self.whole == 0 && self.nano == 0
    }

    pub fn multiplied(&self, rate: f64) -> Self {
        let result = self.value() * rate;
        let abs = result.abs();
        let whole = abs.floor();
        let nano = safe_round((abs - whole) * ONE_STAR_IN_NANO as f64);
        Self::with_nano(
            (if result < 0. { -whole } else { whole }) as i64,
            (if result < 0. { -nano } else { nano }) as i64,
            self.type_(),
        )
    }

    /// As in the original, the result is always of the `Stars` type.
    pub fn abs(&self) -> Self {
        if self.whole < 0 {
            Self::with_nano(-self.whole, -self.nano, CreditsType::Stars)
        } else {
            *self
        }
    }

    fn normalize(&mut self) {
        if self.nano < 0 {
            let shifts = (-self.nano + ONE_STAR_IN_NANO - 1) / ONE_STAR_IN_NANO;
            self.nano += shifts * ONE_STAR_IN_NANO;
            self.whole -= shifts;
        } else if self.nano >= ONE_STAR_IN_NANO {
            let shifts = self.nano / ONE_STAR_IN_NANO;
            self.nano -= shifts * ONE_STAR_IN_NANO;
            self.whole += shifts;
        }
    }
}

/// The hand-written `operator<=>`: the type is compared only for non-empty
/// amounts. (So two empty amounts of different types compare as equal here
/// while `==` tells them apart, exactly as in the original.)
impl PartialOrd for CreditsAmount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(
            self.whole
                .cmp(&other.whole)
                .then(self.nano.cmp(&other.nano))
                .then_with(|| {
                    if self.whole != 0 || self.nano != 0 {
                        self.ton.cmp(&other.ton)
                    } else {
                        Ordering::Equal
                    }
                }),
        )
    }
}

impl AddAssign for CreditsAmount {
    fn add_assign(&mut self, other: Self) {
        self.whole += other.whole;
        self.nano += other.nano;
        self.normalize();
    }
}

impl SubAssign for CreditsAmount {
    fn sub_assign(&mut self, other: Self) {
        self.whole -= other.whole;
        self.nano -= other.nano;
        self.normalize();
    }
}

impl MulAssign<i64> for CreditsAmount {
    fn mul_assign(&mut self, multiplier: i64) {
        self.whole *= multiplier;
        self.nano *= multiplier;
        self.normalize();
    }
}

impl Neg for CreditsAmount {
    type Output = Self;
    fn neg(mut self) -> Self {
        self *= -1;
        self
    }
}

impl Add for CreditsAmount {
    type Output = Self;
    fn add(mut self, other: Self) -> Self {
        self += other;
        self
    }
}

impl Sub for CreditsAmount {
    type Output = Self;
    fn sub(mut self, other: Self) -> Self {
        self -= other;
        self
    }
}

impl Mul<i64> for CreditsAmount {
    type Output = Self;
    fn mul(mut self, multiplier: i64) -> Self {
        self *= multiplier;
        self
    }
}

impl Mul<CreditsAmount> for i64 {
    type Output = CreditsAmount;
    fn mul(self, mut amount: CreditsAmount) -> CreditsAmount {
        amount *= self;
        amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stars(whole: i64, nano: i64) -> CreditsAmount {
        CreditsAmount::with_nano(whole, nano, CreditsType::Stars)
    }

    #[test]
    fn normalization() {
        let amount = stars(1, 2_500_000_000);
        assert_eq!((amount.whole(), amount.nano()), (3, 500_000_000));
        let amount = stars(1, -1);
        assert_eq!((amount.whole(), amount.nano()), (0, 999_999_999));
        let amount = stars(0, -ONE_STAR_IN_NANO);
        assert_eq!((amount.whole(), amount.nano()), (-1, 0));
        let amount = stars(0, -2_500_000_000);
        assert_eq!((amount.whole(), amount.nano()), (-3, 500_000_000));
        assert_eq!(amount.value(), -2.5);
    }

    #[test]
    fn arithmetic() {
        let a = stars(1, 600_000_000);
        let b = stars(2, 700_000_000);
        assert_eq!(a + b, stars(4, 300_000_000));
        assert_eq!(a - b, stars(-2, 900_000_000));
        assert_eq!((a - b).value(), -1.1);
        assert_eq!(a * 3, stars(4, 800_000_000));
        assert_eq!(3 * a, a * 3);
        assert_eq!(-a, stars(-2, 400_000_000));
        assert_eq!((-a).abs(), stars(1, 600_000_000));
        assert_eq!(a.abs(), a);
        assert_eq!(a.multiplied(0.5), stars(0, 800_000_000));
        assert_eq!(a.multiplied(-0.5), stars(-1, 200_000_000));
    }

    #[test]
    fn type_and_emptiness() {
        let ton = CreditsAmount::new(5, CreditsType::Ton);
        assert!(ton.ton() && !ton.stars());
        assert_eq!(ton.type_(), CreditsType::Ton);
        assert_eq!(ton.multiplied(2.).type_(), CreditsType::Ton);
        assert!(CreditsAmount::default().empty());
        assert!(!ton.empty());
        // `abs` of a negative amount loses the type, as in the original.
        assert_eq!(
            CreditsAmount::new(-5, CreditsType::Ton).abs().type_(),
            CreditsType::Stars
        );
        assert_eq!(ton.abs().type_(), CreditsType::Ton);
    }

    #[test]
    fn ordering_ignores_the_type_of_empty_amounts() {
        let empty_stars = CreditsAmount::new(0, CreditsType::Stars);
        let empty_ton = CreditsAmount::new(0, CreditsType::Ton);
        assert_eq!(empty_stars.partial_cmp(&empty_ton), Some(Ordering::Equal));
        assert_ne!(empty_stars, empty_ton);
        let stars = CreditsAmount::new(1, CreditsType::Stars);
        let ton = CreditsAmount::new(1, CreditsType::Ton);
        assert!(stars < ton);
        assert!(stars < CreditsAmount::new(2, CreditsType::Stars));
        assert!(self::stars(1, 1) > stars);
    }
}
