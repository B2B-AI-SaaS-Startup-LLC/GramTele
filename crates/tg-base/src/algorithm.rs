//! Port of `base/algorithm.h` / `algorithm.cpp`.
//!
//! Replaced by the standard library (no code here):
//! `base::take(x)` -> `std::mem::take(&mut x)`, `base::duplicate(x)` ->
//! `x.clone()`, `array_size(a)` -> `a.len()`, `base::contains(c, v)` ->
//! `c.contains(&v)` / `c.iter().any(..)`, `up_cast` -> `From`/`Into` or a
//! trait-object coercion, `qstr("..")` -> `"..."`,
//! `pointer_comparator` -> key sets by `Rc::as_ptr`/`Arc::as_ptr`.

/// `accumulate_max`: `a = max(a, b)` assigning only when `a < b`.
pub fn accumulate_max<T: PartialOrd>(a: &mut T, b: T) {
    if *a < b {
        *a = b;
    }
}

/// `accumulate_min`: `a = min(a, b)` assigning only when `a > b`.
pub fn accumulate_min<T: PartialOrd>(a: &mut T, b: T) {
    if *a > b {
        *a = b;
    }
}

/// `base::reorder`: moves the element at `old_position` to `new_position`
/// shifting the ones in between.
pub fn reorder<T>(container: &mut [T], old_position: usize, new_position: usize) {
    if old_position < new_position {
        container[old_position..=new_position].rotate_left(1);
    } else if new_position < old_position {
        container[new_position..=old_position].rotate_right(1);
    }
}

/// `base::reorder_index`: where an element that was at `index` ends up
/// after `reorder(old_position, new_position)`.
#[must_use]
pub fn reorder_index(index: i32, old_position: i32, new_position: i32) -> i32 {
    if index == old_position {
        new_position
    } else if old_position < index && index <= new_position {
        index - 1
    } else if old_position > index && index >= new_position {
        index + 1
    } else {
        index
    }
}

/// `base::FromUtf8Safe`: valid UTF-8 is decoded as is, otherwise the
/// bytes go through lossy decoding (the original falls back to the local
/// 8-bit codec, which on Windows is the ANSI code page; not reproduced).
pub fn from_utf8_safe(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// `base::SafeRound`: round half away from zero; `value` must not be NaN.
#[must_use]
pub fn safe_round(value: f64) -> f64 {
    crate::expects!(!value.is_nan());
    let result = value.round();
    if result.is_nan() {
        // The original retries around floating point exception state
        // quirks; in Rust `round` of a non-NaN value is never NaN.
        crate::unexpected!("NAN after std::round.");
    }
    result
}

/// `base::CleanAndSimplify`: control characters (< 32) become spaces, then
/// leading/trailing whitespace is trimmed and inner runs collapse to one
/// space (`QString::simplified`).
#[must_use]
pub fn clean_and_simplify(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|ch| if (ch as u32) < 32 { ' ' } else { ch })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `base::OppositeSigns`: false if either is zero.
#[must_use]
pub fn opposite_signs<T: PartialOrd + Default>(a: T, b: T) -> bool {
    let zero = T::default();
    (a < zero && b > zero) || (a > zero && b < zero)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reorder_moves_forward_and_backward() {
        let mut v = [0, 1, 2, 3, 4];
        reorder(&mut v, 1, 3);
        assert_eq!(v, [0, 2, 3, 1, 4]);
        reorder(&mut v, 3, 1);
        assert_eq!(v, [0, 1, 2, 3, 4]);
        reorder(&mut v, 2, 2);
        assert_eq!(v, [0, 1, 2, 3, 4]);
        reorder(&mut v, 0, 4);
        assert_eq!(v, [1, 2, 3, 4, 0]);
    }

    #[test]
    fn reorder_index_matches_reorder() {
        for old in 0..5 {
            for new in 0..5 {
                let mut v: Vec<i32> = (0..5).collect();
                reorder(&mut v, old as usize, new as usize);
                for index in 0..5 {
                    let at = v.iter().position(|&x| x == index).unwrap();
                    assert_eq!(reorder_index(index, old, new), at as i32);
                }
            }
        }
    }

    #[test]
    fn accumulate() {
        let mut a = 3;
        accumulate_max(&mut a, 2);
        assert_eq!(a, 3);
        accumulate_max(&mut a, 5);
        assert_eq!(a, 5);
        accumulate_min(&mut a, 7);
        assert_eq!(a, 5);
        accumulate_min(&mut a, -1);
        assert_eq!(a, -1);
    }

    #[test]
    fn rounding() {
        assert_eq!(safe_round(2.5), 3.);
        assert_eq!(safe_round(-2.5), -3.);
        assert_eq!(safe_round(0.49), 0.);
        assert_eq!(safe_round(f64::INFINITY), f64::INFINITY);
    }

    #[test]
    #[should_panic]
    fn rounding_nan_fails() {
        let _ = safe_round(f64::NAN);
    }

    #[test]
    fn simplify() {
        assert_eq!(clean_and_simplify("  a\tb\n\nc  "), "a b c");
        assert_eq!(clean_and_simplify("\u{1}\u{2}"), "");
        assert_eq!(clean_and_simplify(""), "");
    }

    #[test]
    fn signs() {
        assert!(opposite_signs(-1, 2));
        assert!(opposite_signs(1.5, -0.1));
        assert!(!opposite_signs(0, 5));
        assert!(!opposite_signs(3, 5));
        assert!(!opposite_signs(-3, -5));
    }

    #[test]
    fn utf8() {
        assert_eq!(from_utf8_safe("привет".as_bytes()), "привет");
        assert_eq!(from_utf8_safe(&[b'a', 0xff]), "a\u{fffd}");
        assert_eq!(from_utf8_safe(b""), "");
    }
}
