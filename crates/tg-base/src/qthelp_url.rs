//! Port of `lib_base/base/qthelp_url.h` and `qthelp_url.cpp`.
//!
//! Percent-encoding follows `QByteArray::toPercentEncoding` /
//! `fromPercentEncoding` exactly (including Qt's lax handling of malformed
//! `%` sequences). The domain expressions start with a PCRE look-behind
//! `(?<![\w\$\-\_%=\.])` that the `regex` crate does not support; it is
//! emulated by [`LookbehindRegex`], which checks the character before each
//! candidate start and retries one character later, as PCRE does.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::{Captures, Regex};

/// A regular expression preceded by a single-character negative look-behind.
#[derive(Debug)]
pub struct LookbehindRegex {
    regex: Regex,
    forbidden_before: Regex,
}

impl LookbehindRegex {
    fn new(forbidden_before: &str, pattern: &str) -> Self {
        Self {
            regex: Regex::new(pattern).expect("valid built-in expression"),
            forbidden_before: Regex::new(&format!("^{forbidden_before}$"))
                .expect("valid built-in class"),
        }
    }

    fn allowed_at(&self, haystack: &str, start: usize) -> bool {
        match haystack[..start].chars().next_back() {
            None => true,
            Some(c) => {
                let mut buffer = [0u8; 4];
                !self.forbidden_before.is_match(c.encode_utf8(&mut buffer))
            }
        }
    }

    /// First match starting at or after byte offset `start`
    /// (`QRegularExpression::match(subject, offset)`).
    #[must_use]
    pub fn captures_at<'h>(&self, haystack: &'h str, mut start: usize) -> Option<Captures<'h>> {
        loop {
            let captures = self.regex.captures_at(haystack, start)?;
            let begin = captures.get(0).expect("group 0").start();
            if self.allowed_at(haystack, begin) {
                return Some(captures);
            }
            // Retry from the next character, as PCRE moves the start.
            start = begin + haystack[begin..].chars().next().map_or(1, char::len_utf8);
            if start > haystack.len() {
                return None;
            }
        }
    }

    /// First match in `haystack`.
    #[must_use]
    pub fn captures<'h>(&self, haystack: &'h str) -> Option<Captures<'h>> {
        self.captures_at(haystack, 0)
    }

    /// The expression without the look-behind.
    #[must_use]
    pub fn inner(&self) -> &Regex {
        &self.regex
    }
}

/// `\w` with `UseUnicodePropertiesOption` in PCRE2 10.43+ is
/// `[\p{L}\p{N}\p{Mn}\p{Pc}]`; the rest is `\$\-\_%=\.`.
const DOMAIN_LOOKBEHIND: &str = r"[\p{L}\p{N}\p{Mn}\p{Pc}$\-_%=.]";

/// `ExpressionDomain()`: any domain containing at least one '.', including
/// "file.txt".
const EXPRESSION_DOMAIN: &str =
    r"(?:([a-zA-Z]+)://)?((?:[A-Za-zА-ЯЁа-яё0-9\-_]+\.){1,10}([A-Za-zрф\-\d]{2,22})(:\d+)?)";

/// `ExpressionDomainExplicit()`: any domain with a protocol, including
/// "test://localhost".
const EXPRESSION_DOMAIN_EXPLICIT: &str =
    r"(?:([a-zA-Z]+)://)((?:[A-Za-zА-ЯЁа-яё0-9\-_]+\.){0,10}([A-Za-zрф\-\d]{2,22})(:\d+)?)";

/// `ExpressionIpExplicit()`: any IPv4 with a protocol, including
/// "test://127.0.0.1:1234".
const EXPRESSION_IP_EXPLICIT: &str = r"(?:([a-zA-Z]+)://)(((25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)(:\d+)?)";

static REG_EXP_DOMAIN: LazyLock<LookbehindRegex> =
    LazyLock::new(|| LookbehindRegex::new(DOMAIN_LOOKBEHIND, EXPRESSION_DOMAIN));
static REG_EXP_DOMAIN_EXPLICIT: LazyLock<LookbehindRegex> =
    LazyLock::new(|| LookbehindRegex::new(DOMAIN_LOOKBEHIND, EXPRESSION_DOMAIN_EXPLICIT));
static REG_EXP_IP_EXPLICIT: LazyLock<LookbehindRegex> =
    LazyLock::new(|| LookbehindRegex::new(DOMAIN_LOOKBEHIND, EXPRESSION_IP_EXPLICIT));
static REG_EXP_PROTOCOL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^([a-zA-Z]+)://").expect("valid built-in expression"));

/// `qthelp::RegExpDomain()`.
#[must_use]
pub fn reg_exp_domain() -> &'static LookbehindRegex {
    &REG_EXP_DOMAIN
}

/// `qthelp::RegExpDomainExplicit()`.
#[must_use]
pub fn reg_exp_domain_explicit() -> &'static LookbehindRegex {
    &REG_EXP_DOMAIN_EXPLICIT
}

/// `RegExpIpExplicit()` (defined in the .cpp, not declared in the header).
#[must_use]
pub fn reg_exp_ip_explicit() -> &'static LookbehindRegex {
    &REG_EXP_IP_EXPLICIT
}

/// `qthelp::RegExpProtocol()`.
#[must_use]
pub fn reg_exp_protocol() -> &'static Regex {
    &REG_EXP_PROTOCOL
}

/// `qthelp::IsGoodProtocol`: http, https, tg, ton, tonsite, any case.
#[must_use]
pub fn is_good_protocol(protocol: &str) -> bool {
    ["http", "https", "tg", "ton", "tonsite"]
        .iter()
        .any(|good| protocol.eq_ignore_ascii_case(good))
}

/// `qthelp::url_encode` (`QUrl::toPercentEncoding`): UTF-8, everything but
/// RFC 3986 unreserved characters `A-Z a-z 0-9 - . _ ~` becomes `%XX` with
/// upper-case hex.
#[must_use]
pub fn url_encode(part: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut result = String::with_capacity(part.len());
    for &byte in part.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            result.push(byte as char);
        } else {
            result.push('%');
            result.push(HEX[usize::from(byte >> 4)] as char);
            result.push(HEX[usize::from(byte & 0x0F)] as char);
        }
    }
    result
}

/// `QByteArray::fromPercentEncoding`, including its quirks: a `%` needs two
/// following bytes, and the two bytes are not validated (non-hex bytes are
/// combined as raw signed `char` values).
#[must_use]
pub fn from_percent_encoding(input: &[u8]) -> Vec<u8> {
    fn digit(c: u8) -> i32 {
        match c {
            b'0'..=b'9' => i32::from(c - b'0'),
            b'a'..=b'f' => i32::from(c - b'a' + 10),
            b'A'..=b'F' => i32::from(c - b'A' + 10),
            // `char` is signed in the original.
            _ => i32::from(c as i8),
        }
    }
    let mut result = Vec::with_capacity(input.len());
    let mut i = 0usize;
    while i < input.len() {
        let c = input[i];
        if c == b'%' && i + 2 < input.len() {
            let a = digit(input[i + 1]);
            let b = digit(input[i + 2]);
            i += 2;
            result.push(((a << 4) | b) as u8);
        } else {
            result.push(c);
        }
        i += 1;
    }
    result
}

/// `qthelp::url_decode`: '+' becomes ' ', then percent-decoding and UTF-8
/// (invalid sequences become U+FFFD).
#[must_use]
pub fn url_decode(encoded: &str) -> String {
    let replaced = encoded.replace('+', " ");
    String::from_utf8_lossy(&from_percent_encoding(replaced.as_bytes())).into_owned()
}

/// `qthelp::UrlParamNameTransform`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UrlParamNameTransform {
    #[default]
    NoTransform,
    ToLower,
}

/// `qthelp::url_parse_params`: parses "p1=v1&p2=v2&..". Names are not
/// decoded, values are; the first occurrence of a name wins; parts that
/// start with '=' are skipped; a part without '=' (including an empty part)
/// is a name with an empty value.
#[must_use]
pub fn url_parse_params(
    params: &str,
    transform: UrlParamNameTransform,
) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let transform_name = |name: &str| match transform {
        UrlParamNameTransform::ToLower => name.to_lowercase(),
        UrlParamNameTransform::NoTransform => name.to_owned(),
    };
    for param in params.split('&') {
        match param.find('=') {
            // Skip params without a name (starting with '=').
            Some(0) => {}
            Some(separator) => {
                let name = transform_name(&param[..separator]);
                let value = url_decode(&param[separator + 1..]);
                result.entry(name).or_insert(value);
            }
            None => {
                result.entry(transform_name(param)).or_default();
            }
        }
    }
    result
}

/// `qthelp::url_append_query_or_hash`.
///
/// Kept as in the original: with no '?' a '?' is appended; otherwise '&' is
/// used only when the last '?' comes after the last '#'.
#[must_use]
pub fn url_append_query_or_hash(url: &str, add: &str) -> String {
    let Some(query) = url.rfind('?') else {
        return format!("{url}?{add}");
    };
    let separator = match url.rfind('#') {
        Some(hash) if hash > query => '?',
        _ => '&',
    };
    format!("{url}{separator}{add}")
}

/// `qthelp::is_ipv6`: no '.' and at least one ':'.
#[must_use]
pub fn is_ipv6(ip: &str) -> bool {
    !ip.contains('.') && ip.contains(':')
}

fn captured_start(captures: &Option<Captures<'_>>) -> isize {
    captures
        .as_ref()
        .map_or(-1, |c| c.get(0).expect("group 0").start() as isize)
}

/// `qthelp::validate_url`: returns the trimmed URL (with "http://" added for
/// a bare domain) if it is a domain or IPv4 with a good protocol, otherwise
/// an empty string.
#[must_use]
pub fn validate_url(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let domain_match = reg_exp_domain_explicit().captures(trimmed);
    let ip_match = reg_exp_ip_explicit().captures(trimmed);
    if domain_match.is_none() && ip_match.is_none() {
        let domain = reg_exp_domain().captures(trimmed);
        if captured_start(&domain) != 0 {
            return String::new();
        }
        return format!("http://{trimmed}");
    } else if captured_start(&domain_match) != 0 && captured_start(&ip_match) != 0 {
        return String::new();
    }
    let protocol = reg_exp_protocol()
        .captures(trimmed)
        .expect("an explicit match at 0 starts with a protocol");
    if is_good_protocol(&protocol[1]) {
        trimmed.to_owned()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_like_qt() {
        assert_eq!(url_encode("aZ09-._~"), "aZ09-._~");
        assert_eq!(url_encode("a b&c=d/?"), "a%20b%26c%3Dd%2F%3F");
        assert_eq!(url_encode("привет"), "%D0%BF%D1%80%D0%B8%D0%B2%D0%B5%D1%82");
        assert_eq!(url_encode("+*!'()"), "%2B%2A%21%27%28%29");
        assert_eq!(url_encode(""), "");
    }

    #[test]
    fn decode_like_qt() {
        assert_eq!(url_decode("a+b%20c"), "a b c");
        assert_eq!(url_decode("%D0%BF%d1%80"), "пр");
        // A '%' needs two more bytes.
        assert_eq!(url_decode("100%"), "100%");
        assert_eq!(url_decode("%4"), "%4");
        assert_eq!(url_decode("%41"), "A");
        // Non-hex bytes are combined unchecked: ('z' << 4 | 'z') as u8.
        assert_eq!(
            from_percent_encoding(b"%zz"),
            vec![((122 << 4) | 122) as u8]
        );
        assert_eq!(from_percent_encoding(b"%G1"), vec![((71 << 4) | 1) as u8]);
        // `%2B` stays '+' (replacement happens before decoding).
        assert_eq!(url_decode("%2B"), "+");
    }

    #[test]
    fn parse_params() {
        let params = url_parse_params(
            "a=1&B=x%20y&a=2&=skip&flag&&c=",
            UrlParamNameTransform::NoTransform,
        );
        assert_eq!(params.get("a").unwrap(), "1");
        assert_eq!(params.get("B").unwrap(), "x y");
        assert_eq!(params.get("flag").unwrap(), "");
        assert_eq!(params.get("").unwrap(), "");
        assert_eq!(params.get("c").unwrap(), "");
        assert_eq!(params.len(), 5);
        let lower = url_parse_params("Domain=durov&DOMAIN=x", UrlParamNameTransform::ToLower);
        assert_eq!(lower.get("domain").unwrap(), "durov");
        assert_eq!(lower.len(), 1);
        assert_eq!(
            url_parse_params("", UrlParamNameTransform::NoTransform).len(),
            1
        );
    }

    #[test]
    fn append_query_or_hash() {
        assert_eq!(url_append_query_or_hash("http://a", "x=1"), "http://a?x=1");
        assert_eq!(
            url_append_query_or_hash("http://a?b=2", "x=1"),
            "http://a?b=2&x=1"
        );
        assert_eq!(
            url_append_query_or_hash("http://a?b#c", "x=1"),
            "http://a?b#c?x=1"
        );
        assert_eq!(
            url_append_query_or_hash("http://a#c?b", "x=1"),
            "http://a#c?b&x=1"
        );
        // Only '#' and no '?': a '?' is appended after the hash (sic).
        assert_eq!(
            url_append_query_or_hash("http://a#c", "x=1"),
            "http://a#c?x=1"
        );
    }

    #[test]
    fn ipv6() {
        assert!(is_ipv6("::1"));
        assert!(is_ipv6("2001:db8::1"));
        assert!(!is_ipv6("127.0.0.1"));
        assert!(!is_ipv6("::ffff:1.2.3.4"));
        assert!(!is_ipv6("host"));
    }

    #[test]
    fn protocols() {
        assert!(is_good_protocol("HTTPS"));
        assert!(is_good_protocol("tonsite"));
        assert!(!is_good_protocol("ftp"));
        assert!(!is_good_protocol("javascript"));
    }

    #[test]
    fn validate() {
        assert_eq!(validate_url("  telegram.org  "), "http://telegram.org");
        assert_eq!(
            validate_url("https://telegram.org/a?b"),
            "https://telegram.org/a?b"
        );
        assert_eq!(validate_url("tg://resolve"), "tg://resolve");
        assert_eq!(
            validate_url("http://127.0.0.1:8080/x"),
            "http://127.0.0.1:8080/x"
        );
        assert_eq!(validate_url("ftp://telegram.org"), "");
        assert_eq!(validate_url("javascript://alert(1)"), "");
        assert_eq!(validate_url("hello"), "");
        assert_eq!(validate_url(""), "");
        assert_eq!(validate_url("say telegram.org"), "");
        assert_eq!(validate_url("пример.рф"), "http://пример.рф");
        assert_eq!(validate_url("file.txt"), "http://file.txt");
    }

    #[test]
    fn lookbehind_emulation() {
        let re = reg_exp_domain();
        // Nothing before the start of the subject.
        let m = re.captures("xtelegram.org").unwrap();
        assert_eq!(m.get(0).unwrap().start(), 0);
        // Every later start is preceded by a word character or a dot.
        assert!(re.captures("a=telegram.org").is_none());
        assert!(re.captures("=telegram.org").is_none());
        assert_eq!(
            re.captures(" telegram.org")
                .unwrap()
                .get(0)
                .unwrap()
                .start(),
            1
        );
        assert_eq!(
            re.captures("(telegram.org)")
                .unwrap()
                .get(2)
                .unwrap()
                .as_str(),
            "telegram.org"
        );
        assert!(re.captures("%telegram.org").is_none());
        assert!(re.captures("$telegram.org").is_none());
        assert!(re.captures("é.telegram.org").is_none());
    }
}
