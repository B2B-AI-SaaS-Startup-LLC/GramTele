//! Port of `lib_base/base/openssl_help.h`.
//!
//! The header wraps OpenSSL big numbers (`BIGNUM` + `BN_CTX`), SHA-1/256/512,
//! PBKDF2-HMAC-SHA512 and HMAC-SHA256. AES and RSA are *not* wrapped here: the
//! client calls OpenSSL for them directly in `SourceFiles/mtproto`,
//! `lib_storage` and `passport` (see `docs/crypto-bytes-time.md`).
//!
//! Replacements:
//! * `BigNum` — sign + magnitude over `crypto_bigint::BoxedUint`; modular
//!   exponentiation with an odd modulus goes through Montgomery form
//!   (`BoxedMontyForm::pow`), which runs in constant time with respect to the
//!   exponent value. Primality — `crypto-primes` (BPSW) plus 64 random-base
//!   Miller–Rabin rounds, matching the 64 rounds of `BN_is_prime_ex`.
//! * `openssl::Context` (`BN_CTX`) has no counterpart: the scratch space is
//!   managed by the crate, so the `context` arguments are dropped.
//! * SHA — `sha1`, `sha2`; HMAC — `hmac`; PBKDF2 — `pbkdf2`.

use std::cmp::Ordering;

use crypto_bigint::ctutils::CtSelect;
use crypto_bigint::modular::{BoxedMontyForm, BoxedMontyParams};
use crypto_bigint::{BoxedUint, ConcatenatingMul, Gcd, NonZero, Resize};
use crypto_primes::hazmat::MillerRabin;
use hmac::{KeyInit, Mac};
use sha1::Digest;
use zeroize::Zeroize;

/// `kSha1Size`.
pub const SHA1_SIZE: usize = 20;
/// `kSha256Size`.
pub const SHA256_SIZE: usize = 32;
/// `kSha512Size`.
pub const SHA512_SIZE: usize = 64;

/// `kMillerRabinIterationCount` in `BigNum::isPrime`.
const MILLER_RABIN_ITERATION_COUNT: usize = 64;

/// `openssl::BigNum`: an arbitrary precision signed integer with a sticky
/// "failed" flag, mirroring the C++ wrapper over `BIGNUM`.
///
/// Every operation that the wrapper reports as failed in C++ (failed inputs,
/// negative inputs to modular operations, division by zero, missing inverse)
/// sets the flag instead of returning an error.
#[derive(Clone, Debug)]
pub struct BigNum {
    magnitude: BoxedUint,
    negative: bool,
    failed: bool,
}

impl Drop for BigNum {
    fn drop(&mut self) {
        // BN_clear_free also clears the magnitude before releasing it.
        self.magnitude.zeroize();
    }
}

impl Default for BigNum {
    fn default() -> Self {
        Self {
            magnitude: BoxedUint::zero(),
            negative: false,
            failed: false,
        }
    }
}

fn limb_precision(bits: u32) -> u32 {
    let limb = crypto_bigint::Limb::BITS;
    bits.max(1).div_ceil(limb) * limb
}

fn resize(value: &BoxedUint, bits: u32) -> BoxedUint {
    // Callers only shrink values that are known to fit.
    Resize::resize_unchecked(value, limb_precision(bits))
}

fn widen_pair(a: &BoxedUint, b: &BoxedUint) -> (BoxedUint, BoxedUint) {
    let bits = a.bits_precision().max(b.bits_precision());
    (resize(a, bits), resize(b, bits))
}

fn add_magnitudes(a: &BoxedUint, b: &BoxedUint) -> BoxedUint {
    let bits = a.bits_precision().max(b.bits_precision()) + crypto_bigint::Limb::BITS;
    let (a, b) = (resize(a, bits), resize(b, bits));
    a.wrapping_add(&b)
}

/// `|a| - |b|` for `|a| >= |b|`.
fn sub_magnitudes(a: &BoxedUint, b: &BoxedUint) -> BoxedUint {
    let (a, b) = widen_pair(a, b);
    a.wrapping_sub(&b)
}

fn cmp_magnitudes(a: &BoxedUint, b: &BoxedUint) -> Ordering {
    let (a, b) = widen_pair(a, b);
    a.cmp(&b)
}

/// `value mod modulus` for a non-zero modulus, result at modulus precision.
fn reduce(value: &BoxedUint, modulus: &NonZero<BoxedUint>) -> BoxedUint {
    let bits = value.bits_precision().max(modulus.bits_precision());
    let value = resize(value, bits);
    let modulus_wide = NonZero::new(resize(modulus.as_ref(), bits))
        .into_option()
        .expect("non-zero modulus stays non-zero");
    let remainder = value.rem(&modulus_wide);
    resize(&remainder, modulus.bits_precision())
}

fn mul_magnitudes(a: &BoxedUint, b: &BoxedUint) -> BoxedUint {
    a.concatenating_mul(b)
}

fn low_u64(value: &BoxedUint) -> u64 {
    // Also correct on 32-bit targets, where one limb is only four bytes.
    let encoded = value.to_le_bytes();
    let mut bytes = [0; 8];
    let count = encoded.len().min(bytes.len());
    bytes[..count].copy_from_slice(&encoded[..count]);
    u64::from_le_bytes(bytes)
}

impl BigNum {
    /// `BigNum()`: zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// `BigNum(unsigned int word)`.
    pub fn from_word(word: u32) -> Self {
        let mut result = Self::new();
        result.set_word(word);
        result
    }

    /// `BigNum(bytes::const_span bytes)`: unsigned big-endian bytes, leading
    /// zeroes allowed, empty input is zero (`BN_bin2bn`).
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut result = Self::new();
        result.set_bytes(bytes);
        result
    }

    /// `BigNum::Failed()`.
    pub fn failed_value() -> Self {
        Self {
            magnitude: BoxedUint::zero(),
            negative: false,
            failed: true,
        }
    }

    fn from_parts(magnitude: BoxedUint, negative: bool) -> Self {
        let negative = negative && !bool::from(magnitude.is_zero());
        Self {
            magnitude,
            negative,
            failed: false,
        }
    }

    fn assign(&mut self, value: Self) -> &mut Self {
        *self = value;
        self
    }

    fn fail(&mut self) -> &mut Self {
        self.failed = true;
        self
    }

    pub fn set_word(&mut self, word: u32) -> &mut Self {
        self.assign(Self::from_parts(BoxedUint::from(word), false))
    }

    pub fn set_bytes(&mut self, bytes: &[u8]) -> &mut Self {
        if bytes.is_empty() {
            return self.assign(Self::new());
        }
        // The byte length is public (it is the size of the field on the
        // wire), so choosing the precision from it leaks nothing new.
        let precision = limb_precision((bytes.len() * 8) as u32);
        let magnitude = BoxedUint::from_be_slice(bytes, precision)
            .expect("precision is large enough for the input");
        self.assign(Self::from_parts(magnitude, false))
    }

    fn signed_add(a: &Self, b: &Self) -> Self {
        if a.negative == b.negative {
            return Self::from_parts(add_magnitudes(&a.magnitude, &b.magnitude), a.negative);
        }
        match cmp_magnitudes(&a.magnitude, &b.magnitude) {
            Ordering::Less => {
                Self::from_parts(sub_magnitudes(&b.magnitude, &a.magnitude), b.negative)
            }
            _ => Self::from_parts(sub_magnitudes(&a.magnitude, &b.magnitude), a.negative),
        }
    }

    fn negated(&self) -> Self {
        Self::from_parts(self.magnitude.clone(), !self.negative)
    }

    /// `setAdd` (`BN_add`).
    pub fn set_add(&mut self, a: &Self, b: &Self) -> &mut Self {
        if a.failed || b.failed {
            return self.fail();
        }
        self.assign(Self::signed_add(a, b))
    }

    /// `setSub` (`BN_sub`), the result may be negative.
    pub fn set_sub(&mut self, a: &Self, b: &Self) -> &mut Self {
        if a.failed || b.failed {
            return self.fail();
        }
        self.assign(Self::signed_add(a, &b.negated()))
    }

    /// `setMul` (`BN_mul`).
    pub fn set_mul(&mut self, a: &Self, b: &Self) -> &mut Self {
        if a.failed || b.failed {
            return self.fail();
        }
        self.assign(Self::from_parts(
            mul_magnitudes(&a.magnitude, &b.magnitude),
            a.negative != b.negative,
        ))
    }

    fn modulus(m: &Self) -> Option<NonZero<BoxedUint>> {
        m.magnitude.as_nz_vartime().cloned()
    }

    fn mod_inputs_bad(values: &[&Self]) -> bool {
        values
            .iter()
            .any(|value| value.failed || value.is_negative())
    }

    /// `setModAdd` (`BN_mod_add`): `(a + b) mod m`, all inputs non-negative.
    pub fn set_mod_add(&mut self, a: &Self, b: &Self, m: &Self) -> &mut Self {
        if Self::mod_inputs_bad(&[a, b, m]) {
            return self.fail();
        }
        let Some(modulus) = Self::modulus(m) else {
            return self.fail();
        };
        let sum = add_magnitudes(&a.magnitude, &b.magnitude);
        self.assign(Self::from_parts(reduce(&sum, &modulus), false))
    }

    /// `setModSub` (`BN_mod_sub`): `(a - b) mod m` in `[0, m)`.
    pub fn set_mod_sub(&mut self, a: &Self, b: &Self, m: &Self) -> &mut Self {
        if Self::mod_inputs_bad(&[a, b, m]) {
            return self.fail();
        }
        let Some(modulus) = Self::modulus(m) else {
            return self.fail();
        };
        let difference = Self::signed_add(a, &b.negated());
        let remainder = reduce(&difference.magnitude, &modulus);
        let result = if difference.negative && !bool::from(remainder.is_zero()) {
            sub_magnitudes(modulus.as_ref(), &remainder)
        } else {
            remainder
        };
        self.assign(Self::from_parts(result, false))
    }

    /// `setModMul` (`BN_mod_mul`).
    pub fn set_mod_mul(&mut self, a: &Self, b: &Self, m: &Self) -> &mut Self {
        if Self::mod_inputs_bad(&[a, b, m]) {
            return self.fail();
        }
        let Some(modulus) = Self::modulus(m) else {
            return self.fail();
        };
        let product = mul_magnitudes(&a.magnitude, &b.magnitude);
        self.assign(Self::from_parts(reduce(&product, &modulus), false))
    }

    /// `setModInverse` (`BN_mod_inverse`); fails if `gcd(a, m) != 1`.
    pub fn set_mod_inverse(&mut self, a: &Self, m: &Self) -> &mut Self {
        if Self::mod_inputs_bad(&[a, m]) {
            return self.fail();
        }
        let Some(modulus) = Self::modulus(m) else {
            return self.fail();
        };
        let reduced = reduce(&a.magnitude, &modulus);
        match reduced.invert_mod(&modulus).into_option() {
            Some(inverse) => self.assign(Self::from_parts(inverse, false)),
            None => self.fail(),
        }
    }

    /// `setModExp` (`BN_mod_exp`): `base ^ power mod m`.
    ///
    /// For an odd modulus (every modulus the client uses: DH primes, RSA
    /// moduli) this uses Montgomery exponentiation that is constant-time with respect
    /// to the exponent value and its bit length is bounded by the exponent's
    /// precision (derived from its byte length). For even moduli we compute
    /// both candidates and use the crate's constant-time selection, with a
    /// fixed number of iterations for that same public exponent precision.
    pub fn set_mod_exp(&mut self, base: &Self, power: &Self, m: &Self) -> &mut Self {
        if Self::mod_inputs_bad(&[base, power, m]) {
            return self.fail();
        }
        let Some(modulus) = Self::modulus(m) else {
            return self.fail();
        };
        if bool::from(modulus.as_ref().is_one()) {
            return self.assign(Self::new());
        }
        let reduced = reduce(&base.magnitude, &modulus);
        let result = match modulus.as_ref().as_odd_vartime() {
            Some(odd) => {
                let params = BoxedMontyParams::new(odd.clone());
                BoxedMontyForm::new(reduced, &params)
                    .pow(&power.magnitude)
                    .retrieve()
            }
            None => {
                let mut result = resize(&BoxedUint::one(), modulus.bits_precision());
                let exponent = &power.magnitude;
                for index in (0..exponent.bits_precision()).rev() {
                    let squared = reduce(&mul_magnitudes(&result, &result), &modulus);
                    let multiplied = reduce(&mul_magnitudes(&squared, &reduced), &modulus);
                    result = squared.ct_select(&multiplied, exponent.bit(index));
                }
                result
            }
        };
        self.assign(Self::from_parts(result, false))
    }

    /// `setGcd` (`BN_gcd`).
    pub fn set_gcd(&mut self, a: &Self, b: &Self) -> &mut Self {
        if Self::mod_inputs_bad(&[a, b]) {
            return self.fail();
        }
        let (x, y) = widen_pair(&a.magnitude, &b.magnitude);
        self.assign(Self::from_parts(x.gcd(&y), false))
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        !self.failed && bool::from(self.magnitude.is_zero())
    }

    #[must_use]
    pub fn is_one(&self) -> bool {
        !self.failed && !self.negative && bool::from(self.magnitude.is_one())
    }

    #[must_use]
    pub fn is_negative(&self) -> bool {
        !self.failed && self.negative
    }

    /// `isPrime` (`BN_is_prime_ex` with 64 Miller–Rabin rounds).
    ///
    /// Numbers below 2 (including negative ones) are not prime. The test is
    /// Baillie–PSW from `crypto-primes` followed by 64 Miller–Rabin rounds
    /// with random bases, so a number passes only if it passes both what
    /// OpenSSL runs and BPSW.
    #[must_use]
    pub fn is_prime(&self) -> bool {
        if self.failed || self.negative {
            return false;
        }
        let candidate = &self.magnitude;
        if !crypto_primes::is_prime(crypto_primes::Flavor::Any, candidate) {
            return false;
        }
        let Some(odd) = candidate.as_odd_vartime() else {
            // The only even prime.
            return true;
        };
        let test = MillerRabin::new(odd.clone());
        let mut rng = getrandom::rand_core::UnwrapErr(getrandom::SysRng);
        (0..MILLER_RABIN_ITERATION_COUNT)
            .all(|_| test.test_random_base(&mut rng).is_probably_prime())
    }

    /// `subWord` (`BN_sub_word`).
    pub fn sub_word(&mut self, word: u32) -> &mut Self {
        if self.failed {
            return self;
        }
        let value = Self::signed_add(self, &Self::from_word(word).negated());
        self.assign(value)
    }

    /// `divWord` (`BN_div_word`): divides the magnitude, keeps the sign and
    /// stores the remainder of the magnitude in `remainder` (or `u64::MAX`,
    /// the `(BN_ULONG)-1` error value, if `self` has failed).
    ///
    /// Panics if `word == 0` (the original `Expects`).
    pub fn div_word(&mut self, word: u64, remainder: Option<&mut u64>) -> &mut Self {
        assert!(word != 0, "BigNum::divWord: division by zero");
        let result = if self.failed {
            u64::MAX
        } else {
            let divisor = NonZero::new(resize(&BoxedUint::from(word), 64))
                .into_option()
                .expect("checked above");
            let (a, b) = widen_pair(&self.magnitude, divisor.as_ref());
            let divisor = NonZero::new(b).into_option().expect("checked above");
            let (quotient, rest) = a.div_rem(&divisor);
            let negative = self.negative;
            self.assign(Self::from_parts(quotient, negative));
            low_u64(&rest)
        };
        if result == u64::MAX {
            self.failed = true;
        }
        if let Some(remainder) = remainder {
            *remainder = result;
        }
        self
    }

    /// `countModWord` (`BN_mod_word`): remainder of the magnitude.
    #[must_use]
    pub fn count_mod_word(&self, word: u64) -> u64 {
        assert!(word != 0, "BigNum::countModWord: division by zero");
        if self.failed {
            return u64::MAX;
        }
        let (a, b) = widen_pair(&self.magnitude, &BoxedUint::from(word));
        let divisor = NonZero::new(b).into_option().expect("checked above");
        low_u64(&a.rem(&divisor))
    }

    /// `bitsSize` (`BN_num_bits`), 0 for a failed value.
    #[must_use]
    pub fn bits_size(&self) -> i32 {
        if self.failed {
            0
        } else {
            self.magnitude.bit_width() as i32
        }
    }

    /// `bytesSize` (`BN_num_bytes`).
    #[must_use]
    pub fn bytes_size(&self) -> i32 {
        (self.bits_size() + 7) / 8
    }

    /// `getBytes` (`BN_bn2bin`): minimal big-endian magnitude, the sign is
    /// dropped, zero is empty, a failed value is empty.
    #[must_use]
    pub fn get_bytes(&self) -> Vec<u8> {
        if self.failed {
            return Vec::new();
        }
        let length = self.bytes_size() as usize;
        let all = self.magnitude.to_be_bytes();
        all[all.len() - length..].to_vec()
    }

    #[must_use]
    pub fn failed(&self) -> bool {
        self.failed
    }

    #[must_use]
    pub fn add(a: &Self, b: &Self) -> Self {
        let mut result = Self::new();
        result.set_add(a, b);
        result
    }

    #[must_use]
    pub fn sub(a: &Self, b: &Self) -> Self {
        let mut result = Self::new();
        result.set_sub(a, b);
        result
    }

    #[must_use]
    pub fn mul(a: &Self, b: &Self) -> Self {
        let mut result = Self::new();
        result.set_mul(a, b);
        result
    }

    #[must_use]
    pub fn mod_add(a: &Self, b: &Self, m: &Self) -> Self {
        let mut result = Self::new();
        result.set_mod_add(a, b, m);
        result
    }

    #[must_use]
    pub fn mod_sub(a: &Self, b: &Self, m: &Self) -> Self {
        let mut result = Self::new();
        result.set_mod_sub(a, b, m);
        result
    }

    #[must_use]
    pub fn mod_mul(a: &Self, b: &Self, m: &Self) -> Self {
        let mut result = Self::new();
        result.set_mod_mul(a, b, m);
        result
    }

    #[must_use]
    pub fn mod_inverse(a: &Self, m: &Self) -> Self {
        let mut result = Self::new();
        result.set_mod_inverse(a, m);
        result
    }

    #[must_use]
    pub fn mod_exp(base: &Self, power: &Self, m: &Self) -> Self {
        let mut result = Self::new();
        result.set_mod_exp(base, power, m);
        result
    }

    #[must_use]
    pub fn gcd(a: &Self, b: &Self) -> Self {
        let mut result = Self::new();
        result.set_gcd(a, b);
        result
    }

    /// `Compare` (`BN_cmp`): -1, 0 or 1; a failed `a` is less, then a failed
    /// `b` is greater.
    #[must_use]
    pub fn compare(a: &Self, b: &Self) -> i32 {
        if a.failed {
            return -1;
        } else if b.failed {
            return 1;
        }
        let ordering = match (a.is_negative(), b.is_negative()) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => cmp_magnitudes(&a.magnitude, &b.magnitude),
            (true, true) => cmp_magnitudes(&b.magnitude, &a.magnitude),
        };
        match ordering {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        }
    }

    /// `Div` (`BN_div`): quotient truncated toward zero, remainder has the
    /// sign of `a`. Division by zero marks both outputs failed.
    pub fn div(dv: Option<&mut Self>, rem: Option<&mut Self>, a: &Self, b: &Self) {
        if dv.is_none() && rem.is_none() {
            return;
        }
        let divisor = if a.failed || b.failed {
            None
        } else {
            let (x, y) = widen_pair(&a.magnitude, &b.magnitude);
            NonZero::new(y).into_option().map(|y| (x, y))
        };
        let Some((x, y)) = divisor else {
            if let Some(dv) = dv {
                dv.failed = true;
            }
            if let Some(rem) = rem {
                rem.failed = true;
            }
            return;
        };
        let (quotient, remainder) = x.div_rem(&y);
        if let Some(dv) = dv {
            *dv = Self::from_parts(quotient, a.negative != b.negative);
        }
        if let Some(rem) = rem {
            *rem = Self::from_parts(remainder, a.negative);
        }
    }
}

/// `Sha1(data)`.
#[must_use]
pub fn sha1(data: &[u8]) -> Vec<u8> {
    sha1::Sha1::digest(data).to_vec()
}

/// `Sha1(args...)`: hash of the concatenation of `parts`.
#[must_use]
pub fn sha1_parts<T: AsRef<[u8]>>(parts: &[T]) -> Vec<u8> {
    let mut hasher = sha1::Sha1::new();
    for part in parts {
        hasher.update(part.as_ref());
    }
    hasher.finalize().to_vec()
}

/// `Sha1To(dst, data)`: writes 20 bytes to the start of `dst`; panics if
/// `dst` is shorter (the original `Expects`).
pub fn sha1_to(dst: &mut [u8], data: &[u8]) {
    assert!(dst.len() >= SHA1_SIZE, "Sha1To: destination too small");
    dst[..SHA1_SIZE].copy_from_slice(&sha1::Sha1::digest(data));
}

/// `Sha256(data)`.
#[must_use]
pub fn sha256(data: &[u8]) -> Vec<u8> {
    sha2::Sha256::digest(data).to_vec()
}

/// `Sha256(args...)`.
#[must_use]
pub fn sha256_parts<T: AsRef<[u8]>>(parts: &[T]) -> Vec<u8> {
    let mut hasher = sha2::Sha256::new();
    for part in parts {
        hasher.update(part.as_ref());
    }
    hasher.finalize().to_vec()
}

/// `Sha256To(dst, data)`.
pub fn sha256_to(dst: &mut [u8], data: &[u8]) {
    assert!(dst.len() >= SHA256_SIZE, "Sha256To: destination too small");
    dst[..SHA256_SIZE].copy_from_slice(&sha2::Sha256::digest(data));
}

/// `Sha512(data)`.
#[must_use]
pub fn sha512(data: &[u8]) -> Vec<u8> {
    sha2::Sha512::digest(data).to_vec()
}

/// `Sha512(args...)`.
#[must_use]
pub fn sha512_parts<T: AsRef<[u8]>>(parts: &[T]) -> Vec<u8> {
    let mut hasher = sha2::Sha512::new();
    for part in parts {
        hasher.update(part.as_ref());
    }
    hasher.finalize().to_vec()
}

/// `Sha512To(dst, data)`.
pub fn sha512_to(dst: &mut [u8], data: &[u8]) {
    assert!(dst.len() >= SHA512_SIZE, "Sha512To: destination too small");
    dst[..SHA512_SIZE].copy_from_slice(&sha2::Sha512::digest(data));
}

/// `Pbkdf2Sha512`: 64 bytes of PBKDF2-HMAC-SHA512.
///
/// OpenSSL rejects an iteration count below 1 and leaves the zero-filled
/// output untouched; the port returns 64 zero bytes in that case too.
#[must_use]
pub fn pbkdf2_sha512(password: &[u8], salt: &[u8], iterations: i32) -> Vec<u8> {
    let mut result = vec![0u8; SHA512_SIZE];
    if iterations >= 1 {
        pbkdf2::pbkdf2_hmac::<sha2::Sha512>(password, salt, iterations as u32, &mut result);
    }
    result
}

/// `HmacSha256`.
#[must_use]
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac =
        hmac::Hmac::<sha2::Sha256>::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(value: u64) -> BigNum {
        BigNum::from_bytes(&value.to_be_bytes())
    }

    fn value(number: &BigNum) -> i128 {
        assert!(!number.failed());
        let mut bytes = [0u8; 16];
        let raw = number.get_bytes();
        bytes[16 - raw.len()..].copy_from_slice(&raw);
        let magnitude = i128::from_be_bytes(bytes);
        if number.is_negative() {
            -magnitude
        } else {
            magnitude
        }
    }

    #[test]
    fn bytes_round_trip_is_minimal_big_endian() {
        let n = BigNum::from_bytes(&[0, 0, 0x01, 0x02]);
        assert_eq!(n.get_bytes(), vec![0x01, 0x02]);
        assert_eq!(n.bits_size(), 9);
        assert_eq!(n.bytes_size(), 2);
        assert!(BigNum::from_bytes(&[]).is_zero());
        assert!(BigNum::from_bytes(&[0, 0]).get_bytes().is_empty());
        assert_eq!(BigNum::from_word(1).get_bytes(), vec![1]);
        assert!(BigNum::from_word(1).is_one());
    }

    #[test]
    fn non_limb_lengths_and_large_carry() {
        for length in [1, 7, 8, 9, 31, 255, 256, 257] {
            let bytes = vec![0x81; length];
            let number = BigNum::from_bytes(&bytes);
            assert_eq!(number.bytes_size(), length as i32);
            assert_eq!(number.get_bytes(), bytes);
        }
        let max = BigNum::from_bytes(&[0xff; 128]);
        let mut expected = vec![1];
        expected.extend_from_slice(&[0; 128]);
        assert_eq!(BigNum::add(&max, &num(1)).get_bytes(), expected);
        // The exponent's byte length is public; leading zeros never alter value.
        let mut padded = [0; 65];
        padded[64] = 13;
        assert_eq!(
            value(&BigNum::mod_exp(
                &num(4),
                &BigNum::from_bytes(&padded),
                &num(497)
            )),
            445
        );
        assert_eq!(value(&BigNum::mod_exp(&num(3), &num(64), &num(100))), 81);
        assert_eq!(
            num(u64::MAX - 1).count_mod_word((1u64 << 63) + 1),
            (1u64 << 63) - 3
        );
    }

    #[test]
    fn signed_add_sub() {
        assert_eq!(value(&BigNum::sub(&num(5), &num(7))), -2);
        assert_eq!(
            value(&BigNum::add(&BigNum::sub(&num(5), &num(7)), &num(2))),
            0
        );
        assert!(!BigNum::sub(&num(5), &num(5)).is_negative());
        assert_eq!(value(&BigNum::add(&num(u64::MAX), &num(1))), 1i128 << 64);
        let negative = BigNum::sub(&num(1), &num(10));
        assert_eq!(value(&BigNum::sub(&negative, &num(3))), -12);
        assert_eq!(value(&BigNum::mul(&negative, &num(3))), -27);
        let mut word = BigNum::new();
        word.sub_word(1);
        assert_eq!(value(&word), -1);
    }

    #[test]
    fn compare_signed() {
        let minus = BigNum::sub(&num(1), &num(2));
        assert_eq!(BigNum::compare(&minus, &num(0)), -1);
        assert_eq!(BigNum::compare(&num(3), &num(2)), 1);
        assert_eq!(BigNum::compare(&num(3), &BigNum::from_bytes(&[0, 3])), 0);
        assert_eq!(BigNum::compare(&BigNum::failed_value(), &num(1)), -1);
        assert_eq!(BigNum::compare(&num(1), &BigNum::failed_value()), 1);
    }

    #[test]
    fn modular_known_values() {
        let m = num(97);
        assert_eq!(value(&BigNum::mod_add(&num(90), &num(10), &m)), 3);
        assert_eq!(value(&BigNum::mod_sub(&num(3), &num(10), &m)), 90);
        assert_eq!(value(&BigNum::mod_sub(&num(10), &num(3), &m)), 7);
        assert_eq!(value(&BigNum::mod_mul(&num(50), &num(60), &m)), 3000 % 97);
        // 3 * 65 = 195 = 2 * 97 + 1.
        assert_eq!(value(&BigNum::mod_inverse(&num(3), &m)), 65);
        assert!(BigNum::mod_inverse(&num(4), &num(8)).failed());
        // Fermat: 5^96 = 1 mod 97; 4^13 mod 497 = 445 (classic example).
        assert_eq!(value(&BigNum::mod_exp(&num(5), &num(96), &m)), 1);
        assert_eq!(value(&BigNum::mod_exp(&num(4), &num(13), &num(497))), 445);
        // Even modulus path: 3^5 = 243 = 2 * 100 + 43.
        assert_eq!(value(&BigNum::mod_exp(&num(3), &num(5), &num(100))), 43);
        assert_eq!(value(&BigNum::mod_exp(&num(3), &num(0), &num(100))), 1);
        assert!(BigNum::mod_exp(&num(3), &num(5), &num(1)).is_zero());
        assert!(BigNum::mod_exp(&num(3), &num(5), &num(0)).failed());
        // Base larger than modulus.
        assert_eq!(value(&BigNum::mod_exp(&num(1000), &num(1), &m)), 1000 % 97);
        assert_eq!(value(&BigNum::gcd(&num(1071), &num(462))), 21);
    }

    #[test]
    fn modular_rejects_negative_inputs() {
        let minus = BigNum::sub(&num(1), &num(2));
        assert!(BigNum::mod_exp(&minus, &num(2), &num(7)).failed());
        assert!(BigNum::mod_mul(&num(2), &minus, &num(7)).failed());
        assert!(BigNum::mod_add(&num(2), &num(2), &minus).failed());
        assert!(BigNum::gcd(&minus, &num(2)).failed());
        assert!(BigNum::mod_exp(&BigNum::failed_value(), &num(1), &num(7)).failed());
    }

    #[test]
    fn division() {
        let mut q = BigNum::new();
        let mut r = BigNum::new();
        BigNum::div(Some(&mut q), Some(&mut r), &num(17), &num(5));
        assert_eq!((value(&q), value(&r)), (3, 2));
        let minus = BigNum::sub(&num(0), &num(17));
        BigNum::div(Some(&mut q), Some(&mut r), &minus, &num(5));
        assert_eq!((value(&q), value(&r)), (-3, -2));
        BigNum::div(Some(&mut q), None, &num(17), &num(0));
        assert!(q.failed());

        let mut n = num(23);
        let mut rest = 0u64;
        n.sub_word(1).div_word(2, Some(&mut rest));
        assert_eq!((value(&n), rest), (11, 0));
        assert_eq!(num(1_000_003).count_mod_word(24), 1_000_003 % 24);
    }

    #[test]
    fn primality() {
        for prime in [
            2u64,
            3,
            5,
            7,
            97,
            7919,
            2_147_483_647,
            0xFFFF_FFFF_FFFF_FFC5,
        ] {
            assert!(num(prime).is_prime(), "{prime}");
        }
        // 561 and 2465 are Carmichael numbers, 3215031751 is a strong
        // pseudoprime to bases 2, 3, 5 and 7.
        for composite in [
            0u64,
            1,
            4,
            9,
            561,
            2465,
            3_215_031_751,
            0xFFFF_FFFF_FFFF_FFFF,
        ] {
            assert!(!num(composite).is_prime(), "{composite}");
        }
        assert!(!BigNum::sub(&num(0), &num(7)).is_prime());
    }

    #[test]
    fn telegram_dh_prime_is_safe_prime() {
        // `GoodPrime` from `mtproto_dh_utils.cpp`, the 2048-bit prime the
        // servers send in `messages.getDhConfig`.
        let prime = hex_literal::hex!(
            "C71CAEB9C6B1C9048E6C522F70F13F73980D40238E3E21C14934D037563D930F"
            "48198A0AA7C14058229493D22530F4DBFA336F6E0AC925139543AED44CCE7C37"
            "20FD51F69458705AC68CD4FE6B6B13ABDC9746512969328454F18FAF8C595F64"
            "2477FE96BB2A941D5BCD1D4AC8CC49880708FA9B378E3C4F3A9060BEE67CF9A4"
            "A4A695811051907E162753B56B0F6B410DBA74D8A84B2A14B3144E0EF1284754"
            "FD17ED950D5965B4B9DD46582DB1178D169C6BC465B0D6FF9CA3928FEF5B9AE4"
            "E418FC15E83EBEA0F87FA9FF5EED70050DED2849F47BF959D956850CE929851F"
            "0D8115F635B105EE2E4E15D04B2454BF6F4FADF034B10403119CD8E3B92FCC5B"
        );
        let p = BigNum::from_bytes(&prime);
        assert_eq!(p.bits_size(), 2048);
        assert!(p.is_prime());
        let mut q = p.clone();
        q.sub_word(1).div_word(2, None);
        assert!(q.is_prime());
        // g = 3 requires p mod 3 == 2.
        assert_eq!(p.count_mod_word(3), 2);
        let mut not_prime = p.clone();
        not_prime.sub_word(2);
        assert!(!not_prime.is_prime());
    }

    #[test]
    fn raw_rsa_small_key() {
        // Textbook RSA key: p = 61, q = 53, n = 3233, e = 17, d = 2753.
        // Computed by hand: 65^17 mod 3233 = 2790, 2790^2753 mod 3233 = 65.
        let n = num(3233);
        let encrypted = BigNum::mod_exp(&num(65), &num(17), &n);
        assert_eq!(value(&encrypted), 2790);
        let decrypted = BigNum::mod_exp(&encrypted, &num(2753), &n);
        assert_eq!(value(&decrypted), 65);
        // RSA_NO_PADDING output is left-padded with zeroes to the modulus
        // size; `getBytes` is minimal, so callers pad (as
        // `RSAPublicKey::Private::encrypt` does).
        assert_eq!(encrypted.get_bytes(), vec![0x0A, 0xE6]);
        assert_eq!(BigNum::mod_exp(&num(2), &num(3), &n).get_bytes(), vec![8]);
    }
}
