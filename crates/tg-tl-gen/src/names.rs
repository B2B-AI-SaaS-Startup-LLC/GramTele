//! Name mapping between the C++ names the original generator emits and the
//! Rust names this generator emits.
//!
//! All names derive from the C++ *suffix* `S` — the part after the `MTP`,
//! `MTPD`, `mtpc_`, `MTP_` or `c_` prefix (`normalizedName`: the scheme name
//! with `.` replaced by `_`).
//!
//! | C++                          | Rust                         |
//! |------------------------------|------------------------------|
//! | `MTP<S>` (type, boxed or bare) | `pascal(S)` enum           |
//! | `MTPD<S>` (constructor data) | `pascal(S) + "Data"` struct  |
//! | enum variant for `MTPD<S>`   | `pascal(S)`                  |
//! | `MTP<S>` (method)            | `pascal(S)` struct           |
//! | `mtpc_<S>`                   | `<Rust struct>::ID`          |
//! | `MTP_<S>(...)`               | `mtp_<snake(S)>(...)`        |
//! | `.c_<S>()`                   | `.c_<snake(S)>()`            |
//! | `.v<p>()` / `.is_<p>()`      | `.v<lower(p)>()` / `.is_<lower(p)>()` |
//! | `Flag::f_<p>`                | `F_<UPPER(p)>`               |
//! | field `_<p>`                 | `lower(p)` (+ `_` if keyword) |

/// `pascal`: first letter upper-cased, each `_` removed and the letter after
/// it upper-cased; other letters are kept as they are (`server_DH_params_ok`
/// → `ServerDHParamsOk`, `messages_getHistory` → `MessagesGetHistory`).
pub fn pascal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper = true;
    for c in s.chars() {
        if c == '_' || c == '.' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// `snake`: `_` inserted before an upper-case letter that follows a
/// lower-case letter or digit, or that starts a new word after an acronym
/// (`JSONQuery` → `json_query`); everything lower-cased; existing `_` kept
/// (`inputPeerUser` → `input_peer_user`, `server_DH_params_ok` →
/// `server_dh_params_ok`, `messages_getHistory` → `messages_get_history`).
pub fn snake(s: &str) -> String {
    let chars: Vec<char> = s.chars().map(|c| if c == '.' { '_' } else { c }).collect();
    let mut out = String::with_capacity(s.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let boundary = prev.is_ascii_lowercase()
                || prev.is_ascii_digit()
                || (prev.is_ascii_uppercase() && next.is_some_and(|n| n.is_ascii_lowercase()));
            if boundary && !out.ends_with('_') {
                out.push('_');
            }
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// Rust keywords (strict and reserved, 2021 edition, plus `gen`).
pub const KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen", "union",
];

/// Names a generated type must not take: they would shadow the prelude for
/// code doing `use tg_scheme::*`.
pub const RESERVED_TYPE_NAMES: &[&str] = &[
    "Vec",
    "Option",
    "Some",
    "None",
    "Result",
    "Ok",
    "Err",
    "Box",
    "String",
    "Arc",
    "Default",
    "Clone",
    "Copy",
    "Send",
    "Sync",
    "Sized",
    "Drop",
    "Fn",
    "FnMut",
    "FnOnce",
    "Iterator",
    "IntoIterator",
    "Extend",
    "ToString",
    "ToOwned",
    "From",
    "Into",
    "Eq",
    "PartialEq",
    "Ord",
    "PartialOrd",
    "AsRef",
    "AsMut",
    "Debug",
    "Unpin",
];

/// Field name of parameter `p`: lower-cased, `_` appended to keywords
/// (`type` → `type_`, `A` → `a`, `srp_B` → `srp_b`).
pub fn field(p: &str) -> String {
    let lower = p.to_lowercase();
    if KEYWORDS.contains(&lower.as_str()) {
        format!("{lower}_")
    } else {
        lower
    }
}

/// Accessor of parameter `p` (`v<p>()`): `v` + lower-cased name.
pub fn getter(p: &str) -> String {
    format!("v{}", p.to_lowercase())
}

/// Flag check of a `?true` parameter (`is_<p>()`).
pub fn is_getter(p: &str) -> String {
    format!("is_{}", p.to_lowercase())
}

/// Flag constant of parameter `p` (`Flag::f_<p>`).
pub fn flag_const(p: &str) -> String {
    format!("F_{}", p.to_uppercase())
}

/// Rust name of a boxed type with C++ suffix `s` (`MTP<s>`).
pub fn type_name(s: &str) -> String {
    pascal(s)
}

/// Rust name of the data struct of a constructor with C++ suffix `s`
/// (`MTPD<s>`).
pub fn data_name(s: &str) -> String {
    format!("{}Data", pascal(s))
}

/// Enum variant of a constructor with C++ suffix `s`.
pub fn variant_name(s: &str) -> String {
    pascal(s)
}

/// Rust name of a method struct with C++ suffix `s`.
pub fn function_name(s: &str) -> String {
    pascal(s)
}

/// Rust factory of a constructor with C++ suffix `s` (`MTP_<s>`).
pub fn factory_name(s: &str) -> String {
    format!("mtp_{}", snake(s))
}

/// Rust getter of a constructor with C++ suffix `s` (`c_<s>()`).
pub fn c_getter(s: &str) -> String {
    format!("c_{}", snake(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_case() {
        assert_eq!(pascal("user"), "User");
        assert_eq!(pascal("messages_Messages"), "MessagesMessages");
        assert_eq!(pascal("messages_getHistory"), "MessagesGetHistory");
        assert_eq!(pascal("server_DH_params_ok"), "ServerDHParamsOk");
        assert_eq!(pascal("p_Q_inner_data"), "PQInnerData");
    }

    #[test]
    fn snake_case() {
        assert_eq!(snake("inputPeerUser"), "input_peer_user");
        assert_eq!(snake("server_DH_params_ok"), "server_dh_params_ok");
        assert_eq!(
            snake("updateBotWebhookJSONQuery"),
            "update_bot_webhook_json_query"
        );
        assert_eq!(snake("messages_getHistory"), "messages_get_history");
        assert_eq!(snake("dataJSON"), "data_json");
        assert_eq!(snake("inputCheckPasswordSRP"), "input_check_password_srp");
        assert_eq!(snake("int128"), "int128");
    }

    #[test]
    fn fields() {
        assert_eq!(field("type"), "type_");
        assert_eq!(field("A"), "a");
        assert_eq!(field("srp_B"), "srp_b");
        assert_eq!(getter("first_name"), "vfirst_name");
        assert_eq!(
            flag_const("send_paid_messages_stars"),
            "F_SEND_PAID_MESSAGES_STARS"
        );
    }
}
