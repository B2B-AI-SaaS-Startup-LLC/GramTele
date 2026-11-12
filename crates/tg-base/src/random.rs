//! Port of `lib_base/base/random.h` and `random.cpp`.
//!
//! The original draws from OpenSSL `RAND_bytes` (a DRBG seeded by the OS)
//! and lets callers mix extra entropy with `RAND_seed`. The port reads the
//! operating system generator directly through `getrandom`; see
//! [`random_add_seed`] for what happens to extra entropy.

use bytemuck::Pod;

/// `base::RandomFill`. Panics if the system generator fails (the original
/// `Ensures(result)`).
pub fn random_fill(bytes: &mut [u8]) {
    getrandom::fill(bytes).expect("base::RandomFill: system random generator failed");
}

/// `base::RandomValue<T>()` for any plain-old-data type (the C++ version
/// requires `std::is_trivially_copyable_v<T>`). The value is the native
/// in-memory interpretation of random bytes, like the original
/// `reinterpret_cast`.
#[must_use]
pub fn random_value<T: Pod>() -> T {
    let mut result = T::zeroed();
    random_fill(bytemuck::bytes_of_mut(&mut result));
    result
}

fn random_index_with(count: i32, mut next: impl FnMut() -> u32) -> i32 {
    assert!(count > 0, "base::RandomIndex: count must be positive");

    if count == 1 {
        return 0;
    }
    let count = count as u32;
    let max = (u32::MAX / count) * count;
    loop {
        let random = next();
        if random < max {
            return (random % count) as i32;
        }
    }
}

/// `base::RandomIndex(count)`: uniform value in `0..count` (rejection
/// sampling, no modulo bias). Panics if `count <= 0`.
#[must_use]
pub fn random_index(count: i32) -> i32 {
    random_index_with(count, random_value::<u32>)
}

/// `base::BufferedRandom<T>`: refills a buffer of random values when it
/// runs out.
#[derive(Clone, Debug)]
pub struct BufferedRandom<T: Pod> {
    buffer: Vec<T>,
    index: usize,
}

impl<T: Pod> BufferedRandom<T> {
    /// Panics if `buffer_size == 0` (the original `Expects`).
    pub fn new(buffer_size: usize) -> Self {
        assert!(
            buffer_size > 0,
            "BufferedRandom: buffer size must be positive"
        );
        Self {
            buffer: vec![T::zeroed(); buffer_size],
            index: 0,
        }
    }

    #[must_use]
    #[allow(clippy::should_implement_trait)] // Original API returns T, not Option<T>.
    pub fn next(&mut self) -> T {
        if self.index == self.buffer.len() {
            self.index = 0;
        }
        if self.index == 0 {
            random_fill(bytemuck::cast_slice_mut(&mut self.buffer));
        }
        let result = self.buffer[self.index];
        self.index += 1;
        result
    }
}

/// `base::RandomIndex(count, BufferedRandom<uint32> buffered)`.
///
/// The original takes the buffered generator *by value*, so every call works
/// on a copy and the caller's generator never advances. Callers never call
/// `next()` themselves, so the caller's index stays at zero and each call
/// refills the whole copied buffer. The port keeps that behaviour (it only
/// costs time, the distribution is the same) and clones the generator.
#[must_use]
pub fn random_index_buffered(count: i32, buffered: &BufferedRandom<u32>) -> i32 {
    let mut copy = buffered.clone();
    random_index_with(count, || copy.next())
}

/// `base::RandomAddSeed`.
///
/// In the original this calls OpenSSL `RAND_seed`, which feeds the bytes into
/// the DRBG as additional input. It is called with `secure_random` from the
/// server (`account.password`, `account.passwordSettings`,
/// `account.authorizationForm`). The port has no DRBG of its own: every
/// request is served by the operating system generator, which cannot be
/// seeded from user space, so the bytes are intentionally discarded. This
/// does not lower the strength of the output below the OS generator, which
/// already seeds OpenSSL in the original.
pub fn random_add_seed(bytes: &[u8]) {
    let _ = bytes;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_bounds() {
        for count in [1, 2, 3, 7, 1000, i32::MAX] {
            for _ in 0..100 {
                let value = random_index(count);
                assert!((0..count).contains(&value));
            }
        }
    }

    #[test]
    #[should_panic]
    fn index_zero_panics() {
        let _ = random_index(0);
    }

    #[test]
    fn rejection_sampling() {
        // For count = 7 the accepted range is [0, 4294967295 / 7 * 7), that
        // is [0, 4294967292): both large values are rejected.
        let mut values = [4_294_967_293u32, u32::MAX, 10].into_iter();
        assert_eq!(random_index_with(7, || values.next().unwrap()), 3);
        assert!(values.next().is_none());
        let mut values = [4_294_967_294u32, 4].into_iter();
        // 4294967294 is below max = 4294967295 for count = 5, accepted.
        assert_eq!(random_index_with(5, || values.next().unwrap()), 4);
    }

    #[test]
    fn buffered_refills() {
        let mut buffered = BufferedRandom::<u32>::new(2);
        let a = buffered.next();
        let b = buffered.next();
        let c = buffered.next();
        // Probability of all equal is negligible.
        assert!(!(a == b && b == c));
        let index = random_index_buffered(10, &buffered);
        assert!((0..10).contains(&index));
    }

    #[test]
    fn value_types() {
        let _: u64 = random_value();
        let array: [u8; 32] = random_value();
        assert_ne!(array, [0u8; 32]);
    }
}
