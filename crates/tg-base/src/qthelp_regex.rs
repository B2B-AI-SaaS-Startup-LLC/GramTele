//! Port of `lib_base/base/qthelp_regex.h`.
//!
//! The original compiles a `QRegularExpression` (PCRE2 in UTF mode) on every
//! call and returns its first match. The port uses the `regex` crate and
//! translates the pattern so that the PCRE2 behaviour the client relies on is
//! kept:
//!
//! * without `UseUnicodeProperties`, `\d \w \s \b` (and negations) are ASCII
//!   only in PCRE2, while they are Unicode in `regex` — they are rewritten to
//!   ASCII classes;
//! * escaped ASCII punctuation (`\/`, `\_`, `\-`, `\$`, ...) is rewritten to
//!   `\x{..}` so it is always a literal;
//! * inside a character class, `[`, `&` and `~` are literals in PCRE2 but
//!   class syntax in `regex` — they are escaped;
//! * `DontCapture` turns plain `(` into `(?:`, as `PCRE2_NO_AUTO_CAPTURE`
//!   does (named groups still capture).
//!
//! Not supported by `regex` and therefore never matching (the same outcome as
//! an invalid pattern in Qt): look-ahead / look-behind, back-references,
//! atomic groups, possessive quantifiers, `\Z`, `\G`, `\K`. None of the
//! client's `regex_match` patterns uses them (see the tests below, which
//! compile every pattern the client passes). Known difference: in PCRE2
//! without `Multiline`, `$` also matches before a final `\n`; in `regex` it
//! matches only at the very end.

use std::collections::HashMap;
use std::ops::BitOr;

/// `qthelp::RegExOption` values (same bits as `QRegularExpression`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegExOptions(u32);

impl RegExOptions {
    pub const NONE: Self = Self(0x0000);
    pub const CASE_INSENSITIVE: Self = Self(0x0001);
    pub const DOT_MATCHES_EVERYTHING: Self = Self(0x0002);
    pub const MULTILINE: Self = Self(0x0004);
    pub const EXTENDED_SYNTAX: Self = Self(0x0008);
    pub const INVERTED_GREEDINESS: Self = Self(0x0010);
    pub const DONT_CAPTURE: Self = Self(0x0040);
    pub const USE_UNICODE_PROPERTIES: Self = Self(0x0080);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for RegExOptions {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

const ASCII_CLASSES: [(char, &str); 6] = [
    ('d', "digit"),
    ('D', "^digit"),
    ('w', "word"),
    ('W', "^word"),
    ('s', "space"),
    ('S', "^space"),
];

/// Translates a PCRE2 pattern as used by Qt into `regex` syntax.
#[must_use]
pub fn translate_pattern(pattern: &str, options: RegExOptions) -> String {
    let unicode = options.contains(RegExOptions::USE_UNICODE_PROPERTIES);
    let mut result = String::with_capacity(pattern.len() + 16);
    let mut flags = String::new();
    for (option, flag) in [
        (RegExOptions::CASE_INSENSITIVE, 'i'),
        (RegExOptions::DOT_MATCHES_EVERYTHING, 's'),
        (RegExOptions::MULTILINE, 'm'),
        (RegExOptions::EXTENDED_SYNTAX, 'x'),
        (RegExOptions::INVERTED_GREEDINESS, 'U'),
    ] {
        if options.contains(option) {
            flags.push(flag);
        }
    }
    if !flags.is_empty() {
        result.push_str("(?");
        result.push_str(&flags);
        result.push(')');
    }

    let chars: Vec<char> = pattern.chars().collect();
    let mut in_class = false;
    // Position right after the opening '[' (and '^'), where ']' is literal.
    let mut class_start = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            let e = chars[i + 1];
            i += 2;
            if let Some((_, name)) = ASCII_CLASSES.iter().find(|(x, _)| *x == e) {
                if unicode {
                    result.push('\\');
                    result.push(e);
                } else if in_class {
                    result.push_str(&format!("[:{name}:]"));
                } else {
                    result.push_str(&format!("[[:{name}:]]"));
                }
            } else if (e == 'b' || e == 'B') && !in_class {
                if unicode {
                    result.push('\\');
                    result.push(e);
                } else {
                    result.push_str(&format!("(?-u:\\{e})"));
                }
            } else if e.is_ascii_punctuation() {
                result.push_str(&format!("\\x{{{:X}}}", e as u32));
            } else {
                result.push('\\');
                result.push(e);
            }
            continue;
        }
        if in_class {
            match c {
                ']' if i != class_start => {
                    in_class = false;
                    result.push(']');
                }
                '[' if chars.get(i + 1) == Some(&':') => {
                    // POSIX class, the same syntax in both engines.
                    let close = (i + 2..chars.len().saturating_sub(1))
                        .find(|&j| chars[j] == ':' && chars[j + 1] == ']');
                    match close {
                        Some(j) => {
                            result.extend(&chars[i..j + 2]);
                            i = j + 2;
                            continue;
                        }
                        None => result.push_str("\\["),
                    }
                }
                '[' | '&' | '~' => {
                    result.push('\\');
                    result.push(c);
                }
                '-' if result.ends_with('-') => result.push_str("\\-"),
                _ => result.push(c),
            }
            i += 1;
            continue;
        }
        match c {
            '[' => {
                in_class = true;
                result.push('[');
                i += 1;
                if chars.get(i) == Some(&'^') {
                    result.push('^');
                    i += 1;
                }
                class_start = i;
                continue;
            }
            '(' if options.contains(RegExOptions::DONT_CAPTURE)
                && chars.get(i + 1) != Some(&'?') =>
            {
                result.push_str("(?:");
            }
            _ => result.push(c),
        }
        i += 1;
    }
    result
}

/// Compiles `pattern` with Qt semantics; `None` for an invalid pattern.
#[must_use]
pub fn compile(pattern: &str, options: RegExOptions) -> Option<regex::Regex> {
    regex::Regex::new(&translate_pattern(pattern, options)).ok()
}

/// `qthelp::RegularExpressionMatch`: the first match of a pattern, or an
/// invalid match. Offsets are byte offsets into the subject (Qt reports
/// UTF-16 offsets; both are -1 for a missing group).
#[derive(Clone, Debug)]
pub struct RegularExpressionMatch<'s> {
    subject: &'s str,
    groups: Vec<Option<(usize, usize)>>,
    names: HashMap<String, usize>,
}

impl<'s> RegularExpressionMatch<'s> {
    fn invalid(subject: &'s str) -> Self {
        Self {
            subject,
            groups: Vec::new(),
            names: HashMap::new(),
        }
    }

    /// Builds a match from `regex` captures.
    pub fn from_captures(
        regex: &regex::Regex,
        subject: &'s str,
        captures: Option<regex::Captures<'s>>,
    ) -> Self {
        let Some(captures) = captures else {
            return Self::invalid(subject);
        };
        let groups = (0..captures.len())
            .map(|i| captures.get(i).map(|m| (m.start(), m.end())))
            .collect();
        let names = regex
            .capture_names()
            .enumerate()
            .filter_map(|(i, name)| name.map(|name| (name.to_owned(), i)))
            .collect();
        Self {
            subject,
            groups,
            names,
        }
    }

    /// `valid()` / `hasMatch()`.
    #[must_use]
    pub fn valid(&self) -> bool {
        !self.groups.is_empty()
    }

    #[must_use]
    pub fn has_match(&self) -> bool {
        self.valid()
    }

    /// Number of the last group plus one, 0 for no match.
    #[must_use]
    pub fn captured_len(&self) -> usize {
        self.groups.len()
    }

    /// `captured(n)`: empty for a missing group (Qt returns a null string).
    #[must_use]
    pub fn captured(&self, n: usize) -> &'s str {
        match self.groups.get(n).copied().flatten() {
            Some((start, end)) => &self.subject[start..end],
            None => "",
        }
    }

    /// `captured(name)`.
    #[must_use]
    pub fn captured_name(&self, name: &str) -> &'s str {
        self.names.get(name).map_or("", |&n| self.captured(n))
    }

    /// Whether group `n` took part in the match (Qt: `!captured(n).isNull()`).
    #[must_use]
    pub fn has_captured(&self, n: usize) -> bool {
        self.groups.get(n).copied().flatten().is_some()
    }

    /// `capturedStart(n)` in bytes, -1 if missing.
    #[must_use]
    pub fn captured_start(&self, n: usize) -> isize {
        self.groups
            .get(n)
            .copied()
            .flatten()
            .map_or(-1, |(start, _)| start as isize)
    }

    /// `capturedEnd(n)` in bytes, -1 if missing.
    #[must_use]
    pub fn captured_end(&self, n: usize) -> isize {
        self.groups
            .get(n)
            .copied()
            .flatten()
            .map_or(-1, |(_, end)| end as isize)
    }
}

/// `qthelp::regex_match(string, subject, options)`: compiles `pattern` and
/// returns its first (leftmost) match in `subject`. An invalid pattern gives
/// an invalid match, as in Qt.
pub fn regex_match<'s>(
    pattern: &str,
    subject: &'s str,
    options: RegExOptions,
) -> RegularExpressionMatch<'s> {
    match compile(pattern, options) {
        Some(regex) => {
            let captures = regex.captures(subject);
            RegularExpressionMatch::from_captures(&regex, subject, captures)
        }
        None => RegularExpressionMatch::invalid(subject),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CI: RegExOptions = RegExOptions::CASE_INSENSITIVE;

    #[test]
    fn ascii_digits_without_unicode_properties() {
        assert!(regex_match("^[0-9]+$", "123", RegExOptions::NONE).valid());
        assert!(regex_match("^\\d+$", "123", RegExOptions::NONE).valid());
        // Arabic-Indic digits are \d only with UseUnicodeProperties.
        assert!(!regex_match("^\\d+$", "١٢٣", RegExOptions::NONE).valid());
        assert!(regex_match("^\\d+$", "١٢٣", RegExOptions::USE_UNICODE_PROPERTIES).valid());
        assert!(!regex_match("^[\\w]+$", "привет", RegExOptions::NONE).valid());
        assert!(regex_match("^[\\w]+$", "привет", RegExOptions::USE_UNICODE_PROPERTIES).valid());
        assert!(regex_match("^[^\\d]$", "a", RegExOptions::NONE).valid());
        assert!(regex_match("^[\\D]$", "a", RegExOptions::NONE).valid());
    }

    #[test]
    fn literals_in_classes() {
        assert!(regex_match("^[a&b]+$", "a&b", RegExOptions::NONE).valid());
        assert!(regex_match("^[a~]+$", "~a", RegExOptions::NONE).valid());
        assert!(regex_match("^[[]$", "[", RegExOptions::NONE).valid());
        assert!(regex_match("^[]a]+$", "]a", RegExOptions::NONE).valid());
        assert!(regex_match("^[[:alpha:]]+$", "ab", RegExOptions::NONE).valid());
        assert!(regex_match("^[\\-\\_]+$", "-_", RegExOptions::NONE).valid());
    }

    #[test]
    fn options() {
        assert!(regex_match("^ABC$", "abc", CI).valid());
        assert!(!regex_match("^ABC$", "abc", RegExOptions::NONE).valid());
        assert!(regex_match("^a.b$", "a\nb", RegExOptions::DOT_MATCHES_EVERYTHING).valid());
        assert!(!regex_match("^a.b$", "a\nb", RegExOptions::NONE).valid());
        assert!(regex_match("^b$", "a\nb", RegExOptions::MULTILINE).valid());
        let m = regex_match("(a+)", "aaa", RegExOptions::INVERTED_GREEDINESS);
        assert_eq!(m.captured(1), "a");
        let m = regex_match("(a)(?<n>b)", "ab", RegExOptions::DONT_CAPTURE);
        assert_eq!(m.captured_len(), 2);
        assert_eq!(m.captured_name("n"), "b");
    }

    #[test]
    fn unsupported_constructs_do_not_match() {
        assert!(!regex_match("(?<!a)b", "cb", RegExOptions::NONE).valid());
        assert!(!regex_match("(a)\\1", "aa", RegExOptions::NONE).valid());
        assert!(!regex_match("[", "[", RegExOptions::NONE).valid());
    }

    #[test]
    fn captures_and_offsets() {
        let m = regex_match(
            "^(https?://)?([a-zA-Z0-9\\_]+)\\.t\\.me(/\\d+)?/?(\\?.+)?",
            "https://durov.t.me/12?x=1",
            CI,
        );
        assert!(m.valid());
        assert_eq!(m.captured(2), "durov");
        assert_eq!(m.captured(3), "/12");
        assert_eq!(m.captured(4), "?x=1");
        assert_eq!(m.captured_start(2), 8);
        let m = regex_match("^(a)(x)?$", "a", RegExOptions::NONE);
        assert!(!m.has_captured(2));
        assert_eq!(m.captured(2), "");
        assert_eq!(m.captured_start(2), -1);
        let none = regex_match("x", "a", RegExOptions::NONE);
        assert_eq!(none.captured_start(0), -1);
    }

    #[test]
    fn spotify_named_groups() {
        // `ayu/ayu_url_handlers.cpp`.
        let m = regex_match(
            "^(https?:\\/\\/)?([a-zA-Z0-9_]+)\\.spotify\\.com\\/(?<type>track|album|artist|user|playlist)\\/(?<identifier>[a-zA-Z0-9_\\/]+?)((\\?si=.+)?)$",
            "https://open.spotify.com/user/1185903410/playlist/6YAnJeVC7tgOiocOG23Dd",
            CI,
        );
        assert!(m.valid());
        assert_eq!(m.captured_name("type"), "user");
        assert_eq!(
            m.captured_name("identifier"),
            "1185903410/playlist/6YAnJeVC7tgOiocOG23Dd"
        );
    }

    /// Every pattern the client passes to `qthelp::regex_match` compiles.
    #[test]
    fn client_patterns_compile() {
        let patterns = [
            "^(https?://)?[^/@:]+\\.ton($|/)",
            "^(https?://)?([a-zA-Z0-9\\_]+)\\.t\\.me(/\\d+)?/?(\\?.+)?",
            "^(https?://)?(www\\.)?(telegram\\.(me|dog)|t\\.me)/(.+)$",
            "^\\+([0-9]+)(\\?|$)",
            "^(joinchat/|\\+|\\%20)([a-zA-Z0-9\\.\\_\\-]+)(\\?|$)",
            "^(addlist/)([a-zA-Z0-9\\.\\_\\-]+)(\\?|$)",
            "^(addstickers|addemoji)/([a-zA-Z0-9\\.\\_]+)(\\?|$)",
            "^addtheme/([a-zA-Z0-9\\.\\_]+)(\\?|$)",
            "^setlanguage/([a-zA-Z0-9\\.\\_\\-]+)(\\?|$)",
            "^share/url/?\\?(.+)$",
            "^confirmphone/?\\?(.+)",
            "^iv/?\\?(.+)(#|$)",
            "^socks/?\\?(.+)(#|$)",
            "^(invoice/|\\$)([a-zA-Z0-9_\\-]+)(\\?|#|$)",
            "^bg/([a-zA-Z0-9\\.\\_\\-\\~]+)(\\?(.+)?)?$",
            "^[a-fA-F0-9]{6}^",
            "^[a-fA-F0-9]{6}\\-[a-fA-F0-9]{6}$",
            "^[a-fA-F0-9]{6}(\\~[a-fA-F0-9]{6}){1,3}$",
            "^m/([a-zA-Z0-9\\.\\_\\-]+)(\\?|$)",
            "^newbot/([a-zA-Z0-9\\.\\_]+)(/([a-zA-Z0-9\\.\\_]*))?(/?\\?(.+))?$",
            "^c/(\\-?\\d+)(/?\\?|/?$|/\\d+/?(\\?|$)|/\\d+/\\d+/?(\\?|$))",
            "^/(\\d+)/(\\d+)(/?\\?|/?$)",
            "^(?:ayuSettings|exteraSettings)/?\\?(.+)$",
            "^([a-zA-Z0-9\\.\\_]+)(/?\\?|/?$|/[a-zA-Z0-9\\.\\_\\-]+/?(\\?|$)|/\\d+/?(\\?|$)|/s/(\\d+|live)/?(\\?|$)|/a/\\d+/?(\\?|$)|/c/\\d+/?(\\?|$)|/\\d+/\\d+/?(\\?|$))",
            "^/s/(\\d+|live)(/?\\?|/?$)",
            "^resolve/?\\?(.+)(#|$)",
            "(^|\\.)(telegram\\.(me|dog)|t\\.me)$",
            "(^|\\.)(telegram\\.(org|me|dog)|t\\.me|te\\.?legra\\.ph|graph\\.org|fragment\\.com|telesco\\.pe)$",
            "^[a-zA-Z0-9\\.\\_]+$",
            "_([a-z]{2,3}_[A-Z]{2,3}|\\-[a-z]{2,3})?\\.",
            "^(\\d+\\.\\d+:\\d+)(/|$)",
        ];
        for pattern in patterns {
            assert!(compile(pattern, CI).is_some(), "{pattern}");
            assert!(compile(pattern, RegExOptions::NONE).is_some(), "{pattern}");
        }
        assert!(regex_match(
            "^(joinchat/|\\+|\\%20)([a-zA-Z0-9\\.\\_\\-]+)(\\?|$)",
            "+AbC-d_e",
            CI
        )
        .valid());
        assert_eq!(
            regex_match(
                "_([a-z]{2,3}_[A-Z]{2,3}|\\-[a-z]{2,3})?\\.",
                "lang_pt_BR.strings",
                RegExOptions::NONE
            )
            .captured(1),
            "pt_BR"
        );
        assert!(regex_match("(^|\\.)(telegram\\.(me|dog)|t\\.me)$", "WWW.T.ME", CI).valid());
        // `^[a-fA-F0-9]{6}^` (sic, from `local_url_handlers.cpp`) never
        // matches a non-empty tail in PCRE either.
        assert!(!regex_match("^[a-fA-F0-9]{6}^", "aabbcc", CI).valid());
    }
}
