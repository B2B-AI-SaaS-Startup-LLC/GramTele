use super::{
    data::{Data, Id},
    data_read::input_id_to_utf16,
};
use crate::common::{Error, Result};
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replace {
    pub id: Id,
    pub replacement: String,
    pub words: Vec<String>,
}
fn error(file: &str, text: impl Into<String>) -> Error {
    Error::new(file, 0, 402, format!("Bad data: {}", text.into()))
}
fn code(s: &str) -> bool {
    s.starts_with(':')
        && s.ends_with(':')
        && s.len() > 2
        && s[1..s.len() - 1]
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"+-_".contains(&b))
}
pub fn words(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut word = String::new();
    let chars: Vec<_> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_alphanumeric()
            || (matches!(c, '-' | '+') && chars.get(i + 1).is_some_and(|c| c.is_numeric()))
        {
            word.push(c);
        } else if !word.is_empty() {
            result.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        result.push(word);
    }
    result.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    result
}
fn convert(mut id: String, replacement: &str) -> Option<String> {
    if replacement
        .as_bytes()
        .windows(6)
        .any(|s| s.starts_with(b"_tone") && s[5].is_ascii_digit())
    {
        return None;
    }
    let units: Vec<_> = id.encode_utf16().collect();
    if units.len() == 2
        && units[1] == 0xfe0f
        && (matches!(units[0], 0x23 | 0x2a | 0x2640 | 0x2642 | 0x2695)
            || (0x30..=0x39).contains(&units[0]))
    {
        return None;
    }
    if id.chars().count() == 1
        && id
            .chars()
            .next()
            .is_some_and(|c| matches!(c as u32,0x1f3fb..=0x1f3ff|0x1f1e6..=0x1f1ff))
    {
        return None;
    }
    let extra = match id.as_str() {
        "🤦" | "🤸" | "🤹" | "🤼" | "🤽" => "\u{200d}♂\u{fe0f}",
        "🤷" | "🤾" => "\u{200d}♀\u{fe0f}",
        _ => "",
    };
    id.push_str(extra);
    match id.as_str() {
        "👩\u{200d}❤\u{fe0f}\u{200d}💋\u{200d}👨" => id = "💏".into(),
        "👨\u{200d}👩\u{200d}👦" => id = "👪".into(),
        "👩\u{200d}❤\u{fe0f}\u{200d}👨" => id = "💑".into(),
        _ => {}
    }
    for (from, to) in [
        ("ac", "sh"),
        ("bv", "no"),
        ("cp", "fr"),
        ("dg", "io"),
        ("ea", "es"),
        ("hm", "au"),
        ("mf", "fr"),
        ("sj", "no"),
        ("ta", "sh"),
        ("um", "us"),
    ] {
        let flag = |s: &str| {
            s.bytes()
                .map(|b| char::from_u32(0x1f1e6 + (b - b'a') as u32).unwrap())
                .collect::<String>()
        };
        if id == flag(from) {
            id = flag(to);
            break;
        }
    }
    Some(id)
}
pub fn prepare(bytes: &[u8], file: &str) -> Result<Vec<Replace>> {
    let json = crate::common::qt_json::parse(bytes, file)?;
    let map = json
        .as_object()
        .ok_or_else(|| error(file, "Root object not found."))?;
    let mut result = Vec::new();
    for (key, child) in map {
        let key = crate::common::qt_json::qt_utf8(key);
        let object = child
            .as_object()
            .ok_or_else(|| error(file, "Child object not found."))?;
        let get = |field: &str| {
            object
                .get(&field.encode_utf16().collect::<Vec<_>>())
                .and_then(|v| v.utf16())
                .ok_or_else(|| error(file, format!("Child '{key}' field not found: {field}")))
        };
        let output = get("output")?;
        let _name = get("name")?;
        let raw_replacement = get("alpha_code")?;
        let replacement = crate::common::qt_json::qt_utf8(raw_replacement);
        let raw_aliases = get("aliases")?;
        if raw_replacement.iter().any(|&c| c > 127) || !code(&replacement) {
            return Err(error(
                file,
                format!("Child '{key}' alpha_code invalid: {replacement}"),
            ));
        }
        let mut aliases: Vec<_> = raw_aliases.split(|&c| c == b'|' as u16).collect();
        let shrug: Vec<_> = ":shrug:".encode_utf16().collect();
        if let Some(index) = aliases.iter().position(|s| *s == shrug.as_slice()) {
            aliases.remove(index);
        }
        if aliases.len() == 1 && aliases[0].is_empty() {
            aliases.clear();
        }
        let mut converted = Vec::new();
        for alias in aliases {
            let a = crate::common::qt_json::qt_utf8(alias);
            if alias.iter().any(|&c| c > 127) || !code(&a) {
                return Err(error(file, format!("Child '{key}' alias invalid: {a}")));
            }
            converted.push(a);
        }
        let mut id = String::new();
        for raw in output.split(|&c| c == b'-' as u16) {
            let p = crate::common::qt_json::qt_utf8(raw);
            let cp = if raw.iter().all(|&c| c < 128)
                && !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                u32::from_str_radix(&p, 16).ok().and_then(char::from_u32)
            } else {
                None
            };
            id.push(
                cp.ok_or_else(|| error(file, format!("Child '{key}' output part invalid: {p}")))?,
            );
        }
        let Some(id) = convert(id, &replacement) else {
            continue;
        };
        let id = id.encode_utf16().collect::<Vec<_>>();
        for replacement in
            std::iter::once(replacement.as_str()).chain(converted.iter().map(String::as_str))
        {
            let words = words(replacement);
            if words.is_empty() {
                return Err(error(file, format!("Child '{replacement}' has no words.")));
            }
            result.push(Replace {
                id: id.clone(),
                replacement: replacement.into(),
                words,
            });
        }
    }
    for (id, replacement) in [
        (0xd83ddc4d, ":like:"),
        (0xd83ddc4e, ":dislike:"),
        (0xd83edd14, ":hmm:"),
        (0xd83edd73, ":party:"),
    ] {
        result.push(Replace {
            id: input_id_to_utf16(&[id]),
            replacement: replacement.into(),
            words: words(replacement),
        });
    }
    Ok(result)
}
pub fn check_and_convert(replaces: Vec<Replace>, data: &Data, file: &str) -> Result<Vec<Replace>> {
    let mut sorted = BTreeMap::<Id, Vec<Replace>>::new();
    let male: Vec<_> = "\u{200d}♂\u{fe0f}".encode_utf16().collect();
    let female: Vec<_> = "\u{200d}♀\u{fe0f}".encode_utf16().collect();
    let find = |id: &Id| {
        data.map
            .get(id)
            .or_else(|| {
                data.map.get(
                    &id.iter()
                        .copied()
                        .filter(|&c| c != 0xfe0f)
                        .collect::<Vec<_>>(),
                )
            })
            .copied()
    };
    let total = replaces.len();
    for mut r in replaces {
        let index = if let Some(i) = find(&r.id) {
            Some(i)
        } else if r.id.ends_with(&male) || r.id.ends_with(&female) {
            find(&r.id[..r.id.len() - 3].to_vec())
        } else {
            let mut a = r.id.clone();
            a.extend(&male);
            let mut b = r.id.clone();
            b.extend(&female);
            match (find(&a), find(&b)) {
                (Some(_), Some(_)) => {
                    return Err(error(
                        file,
                        format!(
                            "Replace '{}' ({}) ambiguous.",
                            r.replacement,
                            String::from_utf16_lossy(&r.id)
                        ),
                    ))
                }
                (a, b) => a.or(b),
            }
        };
        let index = index.ok_or_else(|| {
            error(
                file,
                format!(
                    "Replace '{}' ({}) not found.",
                    r.replacement,
                    String::from_utf16_lossy(&r.id)
                ),
            )
        })?;
        r.id = data.list[index].id.clone();
        if data.list[index].postfixed {
            r.id.push(0xfe0f);
        }
        sorted.entry(r.id.clone()).or_default().insert(0, r); // QMultiMap inserts equal keys before previous entries.
    }
    let mut result = Vec::new();
    for category in &data.categories {
        for &i in category {
            let mut id = data.list[i].id.clone();
            if data.list[i].postfixed {
                id.push(0xfe0f);
            }
            if let Some(list) = sorted.remove(&id) {
                result.extend(list);
            }
        }
    }
    if result.len() != total {
        return Err(error(file, "Some were not found."));
    }
    if !sorted.is_empty() {
        return Err(error(file, "Weird."));
    }
    Ok(result)
}
pub fn count_checksum(s: &str) -> u32 {
    let bytes: Vec<_> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
    tg_base::crc32hash::crc32(&bytes) as u32
}
