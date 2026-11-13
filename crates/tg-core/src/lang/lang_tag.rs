//! Port of `lang/lang_tag.{h,cpp}`: plural rules, tag replacement in
//! language values and the count formatting helpers.
//!
//! Differences from the original:
//! * `ChoosePlural` is a global there; here it is a thread-local, like the
//!   rest of the main-thread language state.
//! * `QLocale()` (the system locale) formats grouped numbers in the
//!   original. Locale data is not ported: [`NumberLocale`] holds the three
//!   characters these helpers need and defaults to the `en_US` ones. The
//!   platform layer is expected to set it from the system locale (stage 6).
// Separate branches preserve the correspondence with original plural rules.
#![allow(clippy::if_same_then_else)]
use crate::core::credits_amount::CreditsAmount;
use std::cell::Cell;
use tg_base::algorithm::safe_round;

/// `kTextCommand`.
pub const TEXT_COMMAND: char = '\u{10}';
/// `kTextCommandLangTag`.
pub const TEXT_COMMAND_LANG_TAG: char = '\u{20}';
/// `kTagReplacementSize`, in UTF-16 units (all four are BMP characters).
pub const TAG_REPLACEMENT_SIZE: i32 = 4;
/// `kPluralKeyBaseForCloudValue`.
pub const PLURAL_KEY_BASE_FOR_CLOUD_VALUE: u16 = u16::MAX;

/// `lngtag_count`: how the `{count}` replacement is formatted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountType {
    /// `lt_count`.
    Count,
    /// `lt_count_short`.
    Short,
    /// `lt_count_decimal`.
    Decimal,
}

fn choose_plural_1(_n: i32, _i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    5
}

fn choose_plural_2fil(_n: i32, i: i32, v: i32, _w: i32, f: i32, _t: i32) -> u16 {
    if v == 0 {
        let mod10 = i % 10;
        if i == 1 || i == 2 || i == 3 {
            return 1;
        } else if mod10 != 4 && mod10 != 6 && mod10 != 9 {
            return 1;
        }
        return 5;
    }
    let mod10 = f % 10;
    if mod10 != 4 && mod10 != 6 && mod10 != 9 {
        return 1;
    }
    5
}

fn choose_plural_2tzm(n: i32, _i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if n == 0 || n == 1 {
        return 1;
    } else if (11..=99).contains(&n) {
        return 1;
    }
    5
}

fn choose_plural_2is(_n: i32, i: i32, _v: i32, _w: i32, _f: i32, t: i32) -> u16 {
    if t == 0 {
        let mod10 = i % 10;
        let mod100 = i % 100;
        if mod10 == 1 && mod100 != 11 {
            return 1;
        }
        return 5;
    }
    1
}

fn choose_plural_2mk(_n: i32, i: i32, v: i32, _w: i32, f: i32, _t: i32) -> u16 {
    if v == 0 {
        let mod10 = i % 10;
        let mod100 = i % 100;
        if mod10 == 1 && mod100 != 11 {
            return 1;
        }
    }
    let mod10 = f % 10;
    let mod100 = f % 100;
    if (mod10 == 1) && (mod100 != 11) {
        return 1;
    }
    5
}

fn choose_plural_2ak(n: i32, _i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if n == 0 || n == 1 {
        return 1;
    }
    5
}

fn choose_plural_2am(n: i32, i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if i == 0 || n == 1 {
        return 1;
    }
    5
}

fn choose_plural_2hy(_n: i32, i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if i == 0 || i == 1 {
        return 1;
    }
    5
}

fn choose_plural_2si(n: i32, i: i32, _v: i32, _w: i32, f: i32, _t: i32) -> u16 {
    if n == 0 || n == 1 {
        return 1;
    } else if i == 0 && f == 1 {
        return 1;
    }
    5
}

fn choose_plural_2bh(n: i32, _i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if n == 0 || n == 1 {
        return 1;
    }
    5
}

fn choose_plural_2af(n: i32, _i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if n == 1 {
        return 1;
    }
    5
}

fn choose_plural_2ast(_n: i32, i: i32, v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if i == 1 && v == 0 {
        return 1;
    }
    5
}

fn choose_plural_2da(n: i32, i: i32, _v: i32, _w: i32, _f: i32, t: i32) -> u16 {
    if n == 1 {
        return 1;
    } else if t != 0 && ((i == 0) || (i == 1)) {
        return 1;
    }
    5
}

fn choose_plural_3lv(n: i32, _i: i32, v: i32, _w: i32, f: i32, _t: i32) -> u16 {
    let nmod10 = n % 10;
    let nmod100 = n % 100;
    let fmod10 = f % 10;
    let fmod100 = f % 100;
    if nmod10 == 0 {
        return 0;
    } else if (11..=19).contains(&nmod100) {
        return 0;
    } else if (v == 2) && (11..=19).contains(&fmod100) {
        return 0;
    } else if (nmod10 == 1) && (nmod100 != 11) {
        return 1;
    } else if (v == 2) && (fmod10 == 1) && (fmod100 != 11) {
        return 1;
    } else if (v != 2) && (fmod10 == 1) {
        return 1;
    }
    5
}

fn choose_plural_3ksh(n: i32, _i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if n == 0 {
        return 0;
    } else if n == 1 {
        return 1;
    }
    5
}

fn choose_plural_3lag(n: i32, i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if n == 0 {
        return 0;
    } else if (n != 0) && ((i == 0) || (i == 1)) {
        return 1;
    }
    5
}

fn choose_plural_3kw(n: i32, _i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if n == 1 {
        return 1;
    } else if n == 2 {
        return 2;
    }
    5
}

fn choose_plural_3bs(_n: i32, i: i32, v: i32, _w: i32, f: i32, _t: i32) -> u16 {
    if v == 0 {
        let mod10 = i % 10;
        let mod100 = i % 100;
        if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
            return 3;
        } else if (mod10 == 1) && (mod100 != 11) {
            return 1;
        }
        return 5;
    }
    let mod10 = f % 10;
    let mod100 = f % 100;
    if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
        return 3;
    } else if (mod10 == 1) && (mod100 != 11) {
        return 1;
    }
    5
}

fn choose_plural_3shi(n: i32, i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if i == 0 || n == 1 {
        return 1;
    } else if (2..=10).contains(&n) {
        return 3;
    }
    5
}

fn choose_plural_3mo(n: i32, i: i32, v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if v == 0 {
        let mod100 = n % 100;
        if i == 1 {
            return 1;
        } else if n == 0 {
            return 3;
        } else if (n != 1) && (1..=19).contains(&mod100) {
            return 3;
        }
        return 5;
    }
    3
}

fn choose_plural_4be(n: i32, _i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    let mod10 = n % 10;
    let mod100 = n % 100;
    if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
        return 3;
    } else if (mod10 == 1) && (mod100 != 11) {
        return 1;
    } else if mod10 == 0 {
        return 4;
    } else if (5..=9).contains(&mod10) {
        return 4;
    } else if (11..=14).contains(&mod100) {
        return 4;
    }
    5
}

fn choose_plural_4ru(_n: i32, i: i32, v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if v == 0 {
        let mod10 = i % 10;
        let mod100 = i % 100;
        if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
            return 3;
        } else if (mod10 == 1) && (mod100 != 11) {
            return 1;
        }
        return 4;
    }
    5
}

fn choose_plural_4pl(_n: i32, i: i32, v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if v == 0 {
        if i == 1 {
            return 1;
        }
        let mod10 = i % 10;
        let mod100 = i % 100;
        if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
            return 3;
        } else {
            return 4;
        }
    }
    5
}

fn choose_plural_4lt(n: i32, _i: i32, _v: i32, _w: i32, f: i32, _t: i32) -> u16 {
    let mod10 = n % 10;
    let mod100 = n % 100;
    if (2..=9).contains(&mod10) && !(11..=19).contains(&mod100) {
        return 3;
    } else if (mod10 == 1) && (mod100 != 11) {
        return 1;
    } else if f != 0 {
        return 4;
    }
    5
}

fn choose_plural_4cs(_n: i32, i: i32, v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if v == 0 {
        if i == 1 {
            return 1;
        } else if (2..=4).contains(&i) {
            return 3;
        }
        return 5;
    }
    4
}

fn choose_plural_4gd(n: i32, _i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if n == 1 || n == 11 {
        return 1;
    } else if n == 2 || n == 12 {
        return 2;
    } else if (3..=10).contains(&n) {
        return 3;
    } else if (13..=19).contains(&n) {
        return 3;
    }
    5
}

fn choose_plural_4dsb(_n: i32, i: i32, v: i32, _w: i32, f: i32, _t: i32) -> u16 {
    if v == 0 {
        let imod100 = i % 100;
        if imod100 == 1 {
            return 1;
        } else if imod100 == 2 {
            return 2;
        } else if imod100 == 3 || imod100 == 4 {
            return 3;
        }
    }
    let fmod100 = f % 100;
    if fmod100 == 1 {
        return 1;
    } else if fmod100 == 2 {
        return 2;
    } else if fmod100 == 3 || fmod100 == 4 {
        return 3;
    }
    5
}

fn choose_plural_4sl(_n: i32, i: i32, v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if v == 0 {
        let imod100 = i % 100;
        if imod100 == 3 || imod100 == 4 {
            return 3;
        } else if imod100 == 1 {
            return 1;
        } else if imod100 == 2 {
            return 2;
        }
        return 5;
    }
    3
}

fn choose_plural_4he(n: i32, i: i32, v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if v == 0 {
        if i == 1 {
            return 1;
        } else if i == 2 {
            return 2;
        } else if (n != 0) && (n != 10) && ((n % 10) == 0) {
            return 4;
        }
        return 5;
    }
    5
}

fn choose_plural_4mt(n: i32, _i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    let mod100 = n % 100;
    if n == 1 {
        return 1;
    } else if n == 0 {
        return 3;
    } else if (2..=10).contains(&mod100) {
        return 3;
    } else if (11..=19).contains(&mod100) {
        return 4;
    }
    5
}

fn choose_plural_5gv(_n: i32, i: i32, v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if v == 0 {
        let mod10 = i % 10;
        let mod20 = i % 20;
        if mod10 == 1 {
            return 1;
        } else if mod10 == 2 {
            return 2;
        } else if mod20 == 0 {
            return 3;
        }
        return 5;
    }
    4
}

fn choose_plural_5br(n: i32, _i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    let mod10 = n % 10;
    let mod100 = n % 100;
    if (mod10 == 1) && (mod100 != 11) && (mod100 != 71) && (mod100 != 91) {
        return 1;
    } else if (mod10 == 2) && (mod100 != 12) && (mod100 != 72) && (mod100 != 92) {
        return 2;
    } else if ((mod10 == 3) || (mod10 == 4) || (mod10 == 9))
        && !(10..=19).contains(&mod100)
        && !(70..=79).contains(&mod100)
        && !(90..=99).contains(&mod100)
    {
        return 3;
    } else if (n != 0) && (n % 1000000 == 0) {
        return 4;
    }
    5
}

fn choose_plural_5ga(n: i32, _i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if n == 1 {
        return 1;
    } else if n == 2 {
        return 2;
    } else if (3..=6).contains(&n) {
        return 3;
    } else if (7..=10).contains(&n) {
        return 4;
    }
    5
}

fn choose_plural_6ar(n: i32, _i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if n == 0 {
        return 0;
    } else if n == 1 {
        return 1;
    } else if n == 2 {
        return 2;
    } else if n < 0 {
        return 5;
    }
    let mod100 = n % 100;
    if (3..=10).contains(&mod100) {
        return 3;
    } else if (11..=99).contains(&mod100) {
        return 4;
    }
    5
}

fn choose_plural_6cy(n: i32, _i: i32, _v: i32, _w: i32, _f: i32, _t: i32) -> u16 {
    if n == 0 {
        return 0;
    } else if n == 1 {
        return 1;
    } else if n == 2 {
        return 2;
    } else if n == 3 {
        return 3;
    } else if n == 6 {
        return 4;
    }
    5
}

type Rule = fn(i32, i32, i32, i32, i32, i32) -> u16;

/// A rule together with the name of its function: function pointers are not
/// reliably comparable in Rust, the name stands for `ChoosePlural != ...`.
type NamedRule = (&'static str, Rule);

/// `ChoosePluralDefault`.
const DEFAULT_RULE: NamedRule = ("2ast", choose_plural_2ast);

thread_local! {
    /// `ChoosePlural`.
    static CHOOSE_PLURAL: Cell<NamedRule> = const { Cell::new(DEFAULT_RULE) };
    static NUMBER_LOCALE: Cell<NumberLocale> = const { Cell::new(NumberLocale::EN_US) };
}

/// `ConvertKeyChar`, applied to one UTF-16 unit truncated to `char`.
fn convert_key_char(unit: u16) -> u8 {
    let ch = unit as u8;
    if ch == b'_' {
        b'-'
    } else {
        ch.to_ascii_lowercase()
    }
}

/// `PluralsKey(const char*)`.
fn plural_key(s: &str) -> u64 {
    s.encode_utf16().fold(0u64, |key, unit| {
        (key << 8) | u64::from(convert_key_char(unit))
    })
}

fn find_rule(key: u64) -> Option<NamedRule> {
    RULES
        .iter()
        .find(|(language, _, _)| plural_key(language) == key)
        .map(|(_, name, rule)| (*name, *rule))
}

/// The lookup of `UpdatePluralRules`: the full id, then the part before the
/// first `-` / `_`, then `ChoosePlural1`.
fn rule_for(language_id: &str) -> NamedRule {
    let mut parent = 0u64;
    let mut key = 0u64;
    for unit in language_id.encode_utf16() {
        let converted = convert_key_char(unit);
        if converted == b'-' && parent == 0 {
            parent = key;
        }
        key = (key << 8) | u64::from(converted);
    }
    let found = find_rule(key).or_else(|| if parent != 0 { find_rule(parent) } else { None });
    found.unwrap_or(("1", choose_plural_1))
}

/// The key shift a language gives for the CLDR operands; for tests and tools.
pub fn plural_rule(language: &str, n: i32, i: i32, v: i32, w: i32, f: i32, t: i32) -> u16 {
    (rule_for(language).1)(n, i, v, w, f, t)
}

/// `UpdatePluralRules`.
pub fn update_plural_rules(language_id: &str) {
    CHOOSE_PLURAL.with(|current| current.set(rule_for(language_id)));
}

/// Position of the replacement marker of `tag` in a language value, as a
/// byte index, or `None` (`-1` in the original).
///
/// The scan runs over UTF-16 units exactly like the original, so text that
/// only looks like a marker (a non-BMP character in the tag slot) is treated
/// the same way.
pub fn find_tag_replacement_position(original: &str, tag: u16) -> Option<usize> {
    let mut units: Vec<(u16, usize)> = Vec::with_capacity(original.len());
    let mut buffer = [0u16; 2];
    for (index, ch) in original.char_indices() {
        for unit in ch.encode_utf16(&mut buffer) {
            units.push((*unit, index));
        }
    }
    let size = TAG_REPLACEMENT_SIZE as usize;
    let mut position = 0;
    while position < units.len() {
        if units[position].0 == TEXT_COMMAND as u16 {
            if position + size <= units.len()
                && units[position + 1].0 == TEXT_COMMAND_LANG_TAG as u16
                && units[position + 3].0 == TEXT_COMMAND as u16
            {
                if u32::from(units[position + 2].0) == 0x0020 + u32::from(tag) {
                    return Some(units[position].1);
                }
                position += size;
            } else {
                position += 1;
            }
        } else {
            position += 1;
        }
    }
    None
}

/// Byte length of the four-character marker that starts at `start`.
pub(crate) fn tag_replacement_bytes(original: &str, start: usize) -> usize {
    original[start..]
        .chars()
        .take(TAG_REPLACEMENT_SIZE as usize)
        .map(char::len_utf8)
        .sum()
}

/// `ReplaceTag<QString>::Replace`; `start` is a byte index from
/// [`find_tag_replacement_position`].
pub fn replace_at(original: &str, replacement: &str, start: usize) -> String {
    let end = start + tag_replacement_bytes(original, start);
    let mut result = String::with_capacity(original.len() + replacement.len());
    result.push_str(&original[..start]);
    result.push_str(replacement);
    result.push_str(&original[end..]);
    result
}

/// `ReplaceTag<QString>::Call`.
pub fn replace_tag(original: String, tag: u16, replacement: &str) -> String {
    match find_tag_replacement_position(&original, tag) {
        Some(position) => replace_at(&original, replacement, position),
        None => original,
    }
}

/// `FormatDouble`: six decimals with the trailing zeros and dot removed.
pub fn format_double(value: f64) -> String {
    let mut result = format!("{value:.6}");
    while result.ends_with('0') {
        result.pop();
    }
    if result.ends_with('.') {
        result.pop();
    }
    result
}

/// `NonZeroPartToInt`: the fraction digits without the leading zeros.
fn non_zero_part_to_int(value: &str) -> i32 {
    let zeros = value.bytes().take_while(|byte| *byte == b'0').count();
    if zeros > 0 {
        if zeros < value.len() {
            value[zeros..].parse().unwrap_or(0)
        } else {
            0
        }
    } else if value.is_empty() {
        0
    } else {
        value.parse().unwrap_or(0)
    }
}

/// `ShortenedCount`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShortenedCount {
    pub number: i64,
    pub string: String,
    pub shortened: bool,
}

/// `FormatCountToShort`. As in the original, the rounded `number` of a
/// shortened negative value is positive.
pub fn format_count_to_short(number: i64, only_k: bool) -> ShortenedCount {
    let mut result = ShortenedCount {
        number,
        ..Default::default()
    };
    let abs = number.wrapping_abs();
    let mut shorten = |divider: i64, multiplier: char| {
        let sign = if number > 0 { 1 } else { -1 };
        let rounded = abs / (divider / 10);
        result.string = (sign * rounded / 10).to_string();
        if rounded % 10 != 0 {
            result.string.push('.');
            result.string.push_str(&(rounded % 10).to_string());
        }
        result.string.push(multiplier);
        // Update given number.
        // E.g. 12345 will be 12000.
        result.number = rounded * divider;
        result.shortened = true;
    };
    if !only_k && abs >= 1_000_000_000 {
        shorten(1_000_000_000, 'B');
    } else if !only_k && abs >= 1_000_000 {
        shorten(1_000_000, 'M');
    } else if abs >= 10_000 {
        shorten(1_000, 'K');
    } else {
        result.string = number.to_string();
    }
    result
}

/// The part of `QLocale` the count helpers use. See the module docs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberLocale {
    /// `None` is `QLocale::OmitGroupSeparator`.
    pub group_separator: Option<char>,
    pub decimal_point: char,
    pub zero_digit: char,
}

impl NumberLocale {
    pub const EN_US: Self = Self {
        group_separator: Some(','),
        decimal_point: '.',
        zero_digit: '0',
    };

    fn group(&self, digits: &str) -> String {
        let Some(separator) = self.group_separator else {
            return digits.to_owned();
        };
        let mut result = String::with_capacity(digits.len() + digits.len() / 3);
        for (index, digit) in digits.chars().enumerate() {
            if index != 0 && (digits.len() - index).is_multiple_of(3) {
                result.push(separator);
            }
            result.push(digit);
        }
        result
    }

    fn integer(&self, number: i64) -> String {
        let digits = number.unsigned_abs().to_string();
        let grouped = self.group(&digits);
        if number < 0 {
            format!("-{grouped}")
        } else {
            grouped
        }
    }

    /// `QLocale::toString(double, 'f')` with the default precision of 6.
    fn fixed(&self, number: f64) -> String {
        let text = format!("{:.6}", number.abs());
        let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
        let mut result = String::new();
        if number.is_sign_negative() && number != 0. {
            result.push('-');
        }
        result.push_str(&self.group(whole));
        result.push(self.decimal_point);
        result.push_str(fraction);
        result
    }
}

/// Replaces the number formatting characters of this thread.
pub fn set_number_locale(locale: NumberLocale) {
    NUMBER_LOCALE.with(|current| current.set(locale));
}

/// The number formatting characters of this thread.
pub fn number_locale() -> NumberLocale {
    NUMBER_LOCALE.with(Cell::get)
}

/// `FormatCountDecimal`.
pub fn format_count_decimal(number: i64) -> String {
    number_locale().integer(number)
}

/// `qFuzzyCompare(double, double)`.
fn fuzzy_compare(a: f64, b: f64) -> bool {
    (a - b).abs() * 1_000_000_000_000. <= a.abs().min(b.abs())
}

/// `FormatExactCountDecimal`. As in the original, only the zero digits are
/// stripped from the fraction, so a value that prints as `1.000000` without
/// being "fuzzy equal" to an integer gives `1.`.
pub fn format_exact_count_decimal(number: f64) -> String {
    let locale = number_locale();
    if fuzzy_compare(number, safe_round(number)) {
        return locale.integer(safe_round(number) as i64);
    }
    // Somehow using QLocale::FloatingPointShortest sometimes produces
    // "0.8500000000000001" on some systems / locales,
    // so I want to stick to 6 digits max (default third argument value).
    let mut result = locale.fixed(number);
    while result.ends_with(locale.zero_digit) {
        result.pop();
    }
    result
}

/// `FormatCreditsAmountToShort`.
pub fn format_credits_amount_to_short(amount: CreditsAmount) -> ShortenedCount {
    let attempt = format_count_to_short(amount.whole(), false);
    if attempt.shortened {
        attempt
    } else {
        ShortenedCount {
            string: format_credits_amount_decimal(amount),
            ..Default::default()
        }
    }
}

/// `FormatCreditsAmountDecimal`.
pub fn format_credits_amount_decimal(amount: CreditsAmount) -> String {
    format_exact_count_decimal(amount.value())
}

/// `FormatCreditsAmountRounded`.
pub fn format_credits_amount_rounded(amount: CreditsAmount) -> String {
    let value = amount.value();
    format_exact_count_decimal(safe_round(value * 100.) / 100.)
}

/// `PluralResult`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluralResult {
    pub key_shift: u16,
    pub replacement: String,
}

/// `Plural`. `is_non_default_plural` is `Lang::details::IsNonDefaultPlural`
/// of the current language instance; it is asked only when needed.
pub fn plural_with(
    key_base: u16,
    value: f64,
    type_: CountType,
    is_non_default_plural: impl FnOnce(u16) -> bool,
) -> PluralResult {
    // To correctly select a shift for PluralType::Short
    // we must first round the number.
    let shortened = if type_ == CountType::Short {
        format_count_to_short(i64::from(safe_round(value) as i32), false)
    } else {
        ShortenedCount::default()
    };

    // Simplified.
    let n = if shortened.number != 0 {
        shortened.number as f64
    } else {
        value
    }
    .abs();
    let i = n.floor() as i32;
    let integer = n.ceil() as i32 == i;
    let formatted = if integer {
        String::new()
    } else {
        format_double(n)
    };
    let fraction = formatted
        .split_once('.')
        .map_or("", |(_, fraction)| fraction);
    let v = fraction.len() as i32;
    let w = v;
    let f = non_zero_part_to_int(fraction);
    let t = f;

    let current = CHOOSE_PLURAL.with(Cell::get);
    let use_non_default_plural = current.0 != DEFAULT_RULE.0
        && (key_base == PLURAL_KEY_BASE_FOR_CLOUD_VALUE || is_non_default_plural(key_base));
    let rule = if use_non_default_plural {
        current.1
    } else {
        DEFAULT_RULE.1
    };
    let key_shift = rule(if integer { i } else { -1 }, i, v, w, f, t);
    if integer {
        let round = safe_round(value) as i32;
        let replacement = match type_ {
            CountType::Short => shortened.string,
            CountType::Decimal => format_count_decimal(i64::from(round)),
            CountType::Count => round.to_string(),
        };
        return PluralResult {
            key_shift,
            replacement,
        };
    }
    PluralResult {
        key_shift,
        replacement: format_double(value),
    }
}

/// `Plural` for the current language instance of this thread.
pub fn plural(key_base: u16, value: f64, type_: CountType) -> PluralResult {
    plural_with(key_base, value, type_, |key| {
        super::lang_instance::is_non_default_plural(key)
    })
}

/// `GeneratePluralRulesMap`: language, rule name, rule.
static RULES: &[(&str, &str, Rule)] = &[
    ("fil", "2fil", choose_plural_2fil),
    ("tl", "2fil", choose_plural_2fil),
    ("tzm", "2tzm", choose_plural_2tzm),
    ("is", "2is", choose_plural_2is),
    ("mk", "2mk", choose_plural_2mk),
    ("ak", "2ak", choose_plural_2ak),
    ("guw", "2ak", choose_plural_2ak),
    ("ln", "2ak", choose_plural_2ak),
    ("mg", "2ak", choose_plural_2ak),
    ("nso", "2ak", choose_plural_2ak),
    ("pa", "2ak", choose_plural_2ak),
    ("ti", "2ak", choose_plural_2ak),
    ("wa", "2ak", choose_plural_2ak),
    ("am", "2am", choose_plural_2am),
    ("as", "2am", choose_plural_2am),
    ("bn", "2am", choose_plural_2am),
    ("gu", "2am", choose_plural_2am),
    ("hi", "2am", choose_plural_2am),
    ("kn", "2am", choose_plural_2am),
    ("mr", "2am", choose_plural_2am),
    ("fa", "2am", choose_plural_2am),
    ("zu", "2am", choose_plural_2am),
    ("hy", "2hy", choose_plural_2hy),
    ("fr", "2hy", choose_plural_2hy),
    ("ff", "2hy", choose_plural_2hy),
    ("kab", "2hy", choose_plural_2hy),
    ("pt", "2hy", choose_plural_2hy),
    ("si", "2si", choose_plural_2si),
    ("bh", "2bh", choose_plural_2bh),
    ("bho", "2bh", choose_plural_2bh),
    ("af", "2af", choose_plural_2af),
    ("sq", "2af", choose_plural_2af),
    ("asa", "2af", choose_plural_2af),
    ("az", "2af", choose_plural_2af),
    ("eu", "2af", choose_plural_2af),
    ("bem", "2af", choose_plural_2af),
    ("bez", "2af", choose_plural_2af),
    ("brx", "2af", choose_plural_2af),
    ("bg", "2af", choose_plural_2af),
    ("ckb", "2af", choose_plural_2af),
    ("ce", "2af", choose_plural_2af),
    ("chr", "2af", choose_plural_2af),
    ("cgg", "2af", choose_plural_2af),
    ("dv", "2af", choose_plural_2af),
    ("eo", "2af", choose_plural_2af),
    ("ee", "2af", choose_plural_2af),
    ("fo", "2af", choose_plural_2af),
    ("fur", "2af", choose_plural_2af),
    ("ka", "2af", choose_plural_2af),
    ("el", "2af", choose_plural_2af),
    ("ha", "2af", choose_plural_2af),
    ("haw", "2af", choose_plural_2af),
    ("hu", "2af", choose_plural_2af),
    ("kaj", "2af", choose_plural_2af),
    ("kkj", "2af", choose_plural_2af),
    ("kl", "2af", choose_plural_2af),
    ("ks", "2af", choose_plural_2af),
    ("kk", "2af", choose_plural_2af),
    ("ku", "2af", choose_plural_2af),
    ("ky", "2af", choose_plural_2af),
    ("lb", "2af", choose_plural_2af),
    ("jmc", "2af", choose_plural_2af),
    ("ml", "2af", choose_plural_2af),
    ("mas", "2af", choose_plural_2af),
    ("mgo", "2af", choose_plural_2af),
    ("mn", "2af", choose_plural_2af),
    ("nah", "2af", choose_plural_2af),
    ("ne", "2af", choose_plural_2af),
    ("nnh", "2af", choose_plural_2af),
    ("jgo", "2af", choose_plural_2af),
    ("nd", "2af", choose_plural_2af),
    ("no", "2af", choose_plural_2af),
    ("nb", "2af", choose_plural_2af),
    ("nn", "2af", choose_plural_2af),
    ("ny", "2af", choose_plural_2af),
    ("nyn", "2af", choose_plural_2af),
    ("or", "2af", choose_plural_2af),
    ("om", "2af", choose_plural_2af),
    ("os", "2af", choose_plural_2af),
    ("pap", "2af", choose_plural_2af),
    ("ps", "2af", choose_plural_2af),
    ("rm", "2af", choose_plural_2af),
    ("rof", "2af", choose_plural_2af),
    ("rwk", "2af", choose_plural_2af),
    ("ssy", "2af", choose_plural_2af),
    ("saq", "2af", choose_plural_2af),
    ("seh", "2af", choose_plural_2af),
    ("ksb", "2af", choose_plural_2af),
    ("sn", "2af", choose_plural_2af),
    ("sd", "2af", choose_plural_2af),
    ("xog", "2af", choose_plural_2af),
    ("so", "2af", choose_plural_2af),
    ("nr", "2af", choose_plural_2af),
    ("sdh", "2af", choose_plural_2af),
    ("st", "2af", choose_plural_2af),
    ("es", "2af", choose_plural_2af),
    ("ss", "2af", choose_plural_2af),
    ("gsw", "2af", choose_plural_2af),
    ("syr", "2af", choose_plural_2af),
    ("ta", "2af", choose_plural_2af),
    ("te", "2af", choose_plural_2af),
    ("teo", "2af", choose_plural_2af),
    ("tig", "2af", choose_plural_2af),
    ("ts", "2af", choose_plural_2af),
    ("tn", "2af", choose_plural_2af),
    ("tr", "2af", choose_plural_2af),
    ("tk", "2af", choose_plural_2af),
    ("kcg", "2af", choose_plural_2af),
    ("ug", "2af", choose_plural_2af),
    ("uz", "2af", choose_plural_2af),
    ("ve", "2af", choose_plural_2af),
    ("vo", "2af", choose_plural_2af),
    ("vun", "2af", choose_plural_2af),
    ("wae", "2af", choose_plural_2af),
    ("xh", "2af", choose_plural_2af),
    ("", "2ast", choose_plural_2ast),
    ("ast", "2ast", choose_plural_2ast),
    ("ca", "2ast", choose_plural_2ast),
    ("nl", "2ast", choose_plural_2ast),
    ("en", "2ast", choose_plural_2ast),
    ("et", "2ast", choose_plural_2ast),
    ("pt_PT", "2ast", choose_plural_2ast),
    ("fi", "2ast", choose_plural_2ast),
    ("gl", "2ast", choose_plural_2ast),
    ("lg", "2ast", choose_plural_2ast),
    ("de", "2ast", choose_plural_2ast),
    ("io", "2ast", choose_plural_2ast),
    ("ia", "2ast", choose_plural_2ast),
    ("it", "2ast", choose_plural_2ast),
    ("sc", "2ast", choose_plural_2ast),
    ("scn", "2ast", choose_plural_2ast),
    ("sw", "2ast", choose_plural_2ast),
    ("sv", "2ast", choose_plural_2ast),
    ("ur", "2ast", choose_plural_2ast),
    ("fy", "2ast", choose_plural_2ast),
    ("ji", "2ast", choose_plural_2ast),
    ("yi", "2ast", choose_plural_2ast),
    ("da", "2da", choose_plural_2da),
    ("lv", "3lv", choose_plural_3lv),
    ("prg", "3lv", choose_plural_3lv),
    ("ksh", "3ksh", choose_plural_3ksh),
    ("lag", "3lag", choose_plural_3lag),
    ("kw", "3kw", choose_plural_3kw),
    ("smn", "3kw", choose_plural_3kw),
    ("iu", "3kw", choose_plural_3kw),
    ("smj", "3kw", choose_plural_3kw),
    ("naq", "3kw", choose_plural_3kw),
    ("se", "3kw", choose_plural_3kw),
    ("smi", "3kw", choose_plural_3kw),
    ("sms", "3kw", choose_plural_3kw),
    ("sma", "3kw", choose_plural_3kw),
    ("bs", "3bs", choose_plural_3bs),
    ("hr", "3bs", choose_plural_3bs),
    ("sr", "3bs", choose_plural_3bs),
    ("sh", "3bs", choose_plural_3bs),
    ("sr_Latn", "3bs", choose_plural_3bs),
    ("shi", "3shi", choose_plural_3shi),
    ("mo", "3mo", choose_plural_3mo),
    ("ro_MD", "3mo", choose_plural_3mo),
    ("ro", "3mo", choose_plural_3mo),
    ("be", "4be", choose_plural_4be),
    ("ru", "4ru", choose_plural_4ru),
    ("uk", "4ru", choose_plural_4ru),
    ("pl", "4pl", choose_plural_4pl),
    ("lt", "4lt", choose_plural_4lt),
    ("cs", "4cs", choose_plural_4cs),
    ("sk", "4cs", choose_plural_4cs),
    ("gd", "4gd", choose_plural_4gd),
    ("dsb", "4dsb", choose_plural_4dsb),
    ("hsb", "4dsb", choose_plural_4dsb),
    ("sl", "4sl", choose_plural_4sl),
    ("he", "4he", choose_plural_4he),
    ("iw", "4he", choose_plural_4he),
    ("mt", "4mt", choose_plural_4mt),
    ("gv", "5gv", choose_plural_5gv),
    ("br", "5br", choose_plural_5br),
    ("ga", "5ga", choose_plural_5ga),
    ("ar", "6ar", choose_plural_6ar),
    ("ars", "6ar", choose_plural_6ar),
    ("cy", "6cy", choose_plural_6cy),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_plural_rules_all_languages() {
        let bytes = tg_base::zlib_help::inflate_gzip(
            include_bytes!("../../tests/fixtures/core-base/plurals-native.txt.gz"),
            10 * 1024 * 1024,
        )
        .unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let mut rows = 0;
        for line in text.lines() {
            let fields = line
                .split_whitespace()
                .map(|v| v.parse::<i64>().unwrap())
                .collect::<Vec<_>>();
            let key = fields[0] as u64;
            let (language, _, _) = RULES
                .iter()
                .find(|(lang, _, _)| plural_key(lang) == key)
                .unwrap();
            assert_eq!(
                plural_rule(
                    language,
                    fields[1] as i32,
                    fields[2] as i32,
                    fields[3] as i32,
                    fields[4] as i32,
                    fields[5] as i32,
                    fields[6] as i32
                ),
                fields[7] as u16,
                "{language}: {line}"
            );
            rows += 1;
        }
        assert_eq!(rows, RULES.len() * 221 * 3);
    }

    #[test]
    fn rule_lookup_uses_parent_language_then_the_single_form_rule() {
        assert_eq!(plural_rule("ru-RU", 21, 21, 0, 0, 0, 0), 1);
        assert_eq!(plural_rule("ru_ru", 21, 21, 0, 0, 0, 0), 1);
        assert_eq!(plural_rule("RU", 2, 2, 0, 0, 0, 0), 3);
        assert_eq!(plural_rule("ja", 1, 1, 0, 0, 0, 0), 5);
        // Unknown languages have the single "other" form.
        assert_eq!(rule_for("xx").0, "1");
        assert_eq!(rule_for("xx-YY").0, "1");
        // The empty id is in the table with the default rule.
        assert_eq!(rule_for("").0, DEFAULT_RULE.0);
        assert_eq!(rule_for("en").0, DEFAULT_RULE.0);
        assert_eq!(rule_for("en-GB").0, DEFAULT_RULE.0);
    }

    fn marker(tag: u16) -> String {
        let mut result = String::new();
        result.push(TEXT_COMMAND);
        result.push(TEXT_COMMAND_LANG_TAG);
        result.push(char::from_u32(0x20 + u32::from(tag)).unwrap());
        result.push(TEXT_COMMAND);
        result
    }

    #[test]
    fn tag_position_and_replacement() {
        let value = format!("a{}б{}c", marker(3), marker(200));
        assert_eq!(find_tag_replacement_position(&value, 3), Some(1));
        let second = find_tag_replacement_position(&value, 200).unwrap();
        assert_eq!(&value[..second], format!("a{}б", marker(3)));
        assert_eq!(find_tag_replacement_position(&value, 4), None);
        assert_eq!(
            replace_tag(value.clone(), 200, "😀"),
            format!("a{}б😀c", marker(3))
        );
        assert_eq!(
            replace_tag(value.clone(), 3, ""),
            format!("aб{}c", marker(200))
        );
        assert_eq!(replace_tag(value.clone(), 9, "x"), value);
    }

    #[test]
    fn tag_scan_matches_the_utf16_scan_of_the_original() {
        // A truncated marker at the end is skipped unit by unit.
        let value = "\u{10}\u{20}\u{23}";
        assert_eq!(find_tag_replacement_position(value, 3), None);
        // A marker of another tag is skipped as a whole: the command inside
        // it does not start a new marker.
        let value = format!("\u{10}\u{20}\u{10}\u{10}{}", marker(1));
        assert_eq!(find_tag_replacement_position(&value, 1), Some(4));
        // A non-BMP character in the tag slot takes two units, so the unit at
        // +3 is its low surrogate, not the closing command.
        let value = format!("\u{10}\u{20}😀\u{10}{}", marker(1));
        let position = find_tag_replacement_position(&value, 1).unwrap();
        assert_eq!(&value[position..], marker(1));
    }

    #[test]
    fn count_to_short() {
        let short = |number, only_k| format_count_to_short(number, only_k);
        assert_eq!(short(9_999, false).string, "9999");
        assert!(!short(9_999, false).shortened);
        assert_eq!(short(10_000, false).string, "10K");
        assert_eq!(short(12_345, false).string, "12.3K");
        // The original multiplies the tenths by the whole divider.
        assert_eq!(short(12_345, false).number, 123_000);
        assert_eq!(short(1_250_000, false).string, "1.2M");
        assert_eq!(short(1_250_000, true).string, "1250K");
        assert_eq!(short(3_000_000_000, false).string, "3B");
        assert_eq!(short(-12_345, false).string, "-12.3K");
        // The original keeps the rounded number positive.
        assert_eq!(short(-12_345, false).number, 123_000);
        // Integer division truncates the sign-carrying tenths towards zero.
        assert_eq!(short(-10_900, false).string, "-10.9K");
    }

    #[test]
    fn decimal_formats() {
        assert_eq!(format_double(1.5), "1.5");
        assert_eq!(format_double(2.0), "2");
        assert_eq!(format_double(0.1234567), "0.123457");
        assert_eq!(non_zero_part_to_int(""), 0);
        assert_eq!(non_zero_part_to_int("000"), 0);
        assert_eq!(non_zero_part_to_int("05"), 5);
        assert_eq!(non_zero_part_to_int("25"), 25);

        assert_eq!(format_count_decimal(0), "0");
        assert_eq!(format_count_decimal(999), "999");
        assert_eq!(format_count_decimal(1_000), "1,000");
        assert_eq!(format_count_decimal(-1_234_567), "-1,234,567");
        assert_eq!(format_exact_count_decimal(1_234.0), "1,234");
        assert_eq!(format_exact_count_decimal(1_234.5), "1,234.5");
        assert_eq!(format_exact_count_decimal(-0.25), "-0.25");
        // Not "fuzzy equal" to 1, but prints as 1.000000: the dot stays.
        assert_eq!(format_exact_count_decimal(1.0000001), "1.");

        set_number_locale(NumberLocale {
            group_separator: Some('\u{a0}'),
            decimal_point: ',',
            zero_digit: '0',
        });
        assert_eq!(format_count_decimal(1_234_567), "1\u{a0}234\u{a0}567");
        assert_eq!(format_exact_count_decimal(1_234.5), "1\u{a0}234,5");
        set_number_locale(NumberLocale {
            group_separator: None,
            ..NumberLocale::EN_US
        });
        assert_eq!(format_count_decimal(1_234_567), "1234567");
        set_number_locale(NumberLocale::EN_US);
    }

    #[test]
    fn credits_formats() {
        let amount = CreditsAmount::with_nano(12, 500_000_000, Default::default());
        assert_eq!(format_credits_amount_decimal(amount), "12.5");
        assert_eq!(format_credits_amount_to_short(amount).string, "12.5");
        assert!(!format_credits_amount_to_short(amount).shortened);
        assert_eq!(format_credits_amount_to_short(amount).number, 0);
        let big = CreditsAmount::new(25_000, Default::default());
        assert_eq!(format_credits_amount_to_short(big).string, "25K");
        let odd = CreditsAmount::with_nano(1, 234_567_890, Default::default());
        assert_eq!(format_credits_amount_rounded(odd), "1.23");
    }

    #[test]
    fn plural_uses_the_default_rule_until_the_key_is_translated() {
        update_plural_rules("ru");
        // Not translated: the default (English) rule.
        let english = plural_with(10, 2., CountType::Count, |_| false);
        assert_eq!(english.key_shift, 5);
        assert_eq!(english.replacement, "2");
        assert_eq!(
            plural_with(10, 1., CountType::Count, |_| false).key_shift,
            1
        );
        // Translated: the Russian rule.
        assert_eq!(plural_with(10, 2., CountType::Count, |_| true).key_shift, 3);
        assert_eq!(plural_with(10, 5., CountType::Count, |_| true).key_shift, 4);
        assert_eq!(
            plural_with(10, 21., CountType::Count, |_| true).key_shift,
            1
        );
        // Cloud values never ask the instance.
        let cloud = plural_with(
            PLURAL_KEY_BASE_FOR_CLOUD_VALUE,
            2.,
            CountType::Count,
            |_| panic!("must not be asked"),
        );
        assert_eq!(cloud.key_shift, 3);

        // With the default rule selected the instance is not asked either.
        update_plural_rules("en");
        let result = plural_with(10, 2., CountType::Count, |_| panic!("must not be asked"));
        assert_eq!(result.key_shift, 5);
    }

    #[test]
    fn plural_replacement_formats() {
        update_plural_rules("en");
        let get = |value, type_| plural_with(1, value, type_, |_| false);
        assert_eq!(get(12_345., CountType::Count).replacement, "12345");
        assert_eq!(get(12_345., CountType::Decimal).replacement, "12,345");
        assert_eq!(get(12_345., CountType::Short).replacement, "12.3K");
        assert_eq!(get(-3., CountType::Count).replacement, "-3");
        assert_eq!(get(-1., CountType::Count).key_shift, 1);
        // Fractions print as is and select by the fraction operands.
        let fraction = get(1.5, CountType::Count);
        assert_eq!(fraction.replacement, "1.5");
        assert_eq!(fraction.key_shift, 5);
        assert_eq!(get(1.5, CountType::Decimal).replacement, "1.5");
        // The short form selects the plural by the rounded, shortened number.
        assert_eq!(get(1_000_000., CountType::Short).replacement, "1M");
    }
}
