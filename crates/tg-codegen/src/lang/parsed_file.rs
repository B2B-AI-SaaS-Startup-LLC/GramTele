use crate::common::{Kind, Result, TokenizedFile};
pub const PLURAL_PARTS: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];
pub const PLURAL_TAGS: [&str; 3] = ["count", "count_short", "count_decimal"];
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub value: Vec<u16>,
    pub key_base: String,
    pub tags: Vec<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LangPack {
    pub entries: Vec<Entry>,
    pub tags: Vec<String>,
}
fn valid(text: &str, key: bool) -> bool {
    !text.is_empty()
        && !text.contains("__")
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || (key && b".-".contains(&b)))
}
fn command(index: usize) -> [u16; 4] {
    [0x10, 0x20, 0x20u16.wrapping_add(index as u16), 0x10]
}
pub fn parse(bytes: &[u8], file: &str, known_tags: &[String]) -> Result<LangPack> {
    let mut f = TokenizedFile::parse_lazy(bytes, file)?;
    let mut result = LangPack {
        tags: known_tags.to_vec(),
        ..Default::default()
    };
    while f.peek().kind != Kind::End {
        let key = f.expect(Kind::String)?.value;
        let (base, plural) = match key.split_once('#') {
            Some((base, part))
                if part.eq_ignore_ascii_case("one") || part.eq_ignore_ascii_case("other") =>
            {
                (
                    base,
                    Some(
                        PLURAL_PARTS
                            .iter()
                            .position(|p| *p == part)
                            .ok_or_else(|| {
                                f.error(806, format!("bad plural part for key '{key}': '{part}'"))
                            })?,
                    ),
                )
            }
            None => (key.as_str(), None),
            _ => return Err(f.unexpected("string key name (/^[a-z0-9_.-]+(#(one|other))?$/i)")),
        };
        if !valid(base, true) {
            return Err(f.unexpected("string key name (/^[a-z0-9_.-]+(#(one|other))?$/i)"));
        }
        if !f.eat(Kind::Symbol('=')) {
            return Err(f.unexpected(&format!("'=' for '{key}' key")));
        }
        if f.peek().kind != Kind::String {
            return Err(f.unexpected(&format!("string value for '{key}' key")));
        }
        let value = f.take().value;
        f.expect(Kind::Symbol(';'))?;
        if result
            .entries
            .iter()
            .any(|e| e.key == base && (e.key_base.is_empty() || !e.tags.is_empty()))
        {
            return Err(f.error(806, format!("duplicate found for key '{base}'")));
        }
        let mut entry = Entry {
            key: base.into(),
            ..Default::default()
        };
        let mut extra = Vec::new();
        let mut rest = value.as_str();
        while let Some(start) = rest.find('{') {
            entry.value.extend(rest[..start].encode_utf16());
            rest = &rest[start + 1..];
            let end = rest
                .find('}')
                .ok_or_else(|| f.error(806, "unexpected end of value, end of tag expected."))?;
            let text = &rest[..end];
            let (tag, numeric) = match text.split_once(':') {
                Some((tag, numeric)) if !tag.is_empty() => (tag, Some(numeric)),
                _ => (text, None),
            };
            if !valid(tag, false) {
                return Err(f.error(806, format!("bad tag characters: '{text}'")));
            }
            if entry.tags.iter().any(|t| t == tag) {
                return Err(f.error(806, format!("duplicate found for tag '{text}'")));
            }
            let index = match result.tags.iter().position(|t| t == tag) {
                Some(i) => i,
                None => {
                    result.tags.push(tag.into());
                    result.tags.len() - 1
                }
            };
            if let Some(parts) = numeric {
                let parts: Vec<_> = parts.split('|').collect();
                if parts.len() != 3 {
                    return Err(f.error(
                        806,
                        format!("bad option count for plural key part in tag: '{text}'"),
                    ));
                }
                for (i, part) in parts.iter().enumerate() {
                    if part.matches('#').count() > 1 {
                        return Err(f.error(
                            806,
                            format!(
                                "bad option for plural key part in tag: '{text}', too many '#'."
                            ),
                        ));
                    }
                    let mut v = Vec::new();
                    for (c, s) in part.split('#').enumerate() {
                        if c > 0 {
                            v.extend(command(index));
                        }
                        v.extend(s.encode_utf16());
                    }
                    extra.push(Entry {
                        key: format!("{base}__{tag}{i}"),
                        value: v,
                        ..Default::default()
                    });
                }
            }
            entry.value.extend(command(index));
            entry.tags.push(tag.into());
            rest = &rest[end + 1..];
        }
        entry.value.extend(rest.encode_utf16());
        if let Some(index) = plural {
            if !extra.is_empty() {
                return Err(f.error(666, "internal - "));
            }
            let offset = match result.entries.iter().position(|e| e.key_base == base) {
                Some(i) => i,
                None => {
                    let i = result.entries.len();
                    for n in 0..6 {
                        result.entries.push(Entry {
                            key: format!("{base}__plural{n}"),
                            key_base: base.into(),
                            ..Default::default()
                        });
                    }
                    i
                }
            };
            if !result.entries[offset + index].tags.is_empty() {
                return Err(f.error(
                    806,
                    format!("duplicate found for key '{base}__plural{index}'"),
                ));
            }
            entry.key = format!("{base}__plural{index}");
            entry.key_base = base.into();
            if !entry.tags.iter().any(|t| t == "count") {
                entry.tags.insert(0, "count".into());
            } else {
                entry.tags.retain(|t| t != "count");
                entry.tags.insert(0, "count".into());
            }
            result.entries[offset + index] = entry;
        } else {
            if result.entries.iter().any(|e| e.key == base) {
                return Err(f.error(806, format!("duplicate found for key '{base}'")));
            }
            for tag in &entry.tags {
                if PLURAL_TAGS.contains(&tag.as_str()) {
                    return Err(f.error(
                        806,
                        format!("plural tag '{tag}' used in non-plural key '{base}'"),
                    ));
                }
            }
            result.entries.push(entry);
            result.entries.extend(extra);
        }
    }
    f.finish()?;
    let mut i = 0;
    while i < result.entries.len() {
        if result.entries[i].key_base.is_empty() {
            i += 1;
            continue;
        }
        // Retain original strict assertion: a plural group must not be last.
        if i + 6 >= result.entries.len() {
            return Err(f.error(666, "internal - "));
        }
        let mut tags = Vec::new();
        for e in &result.entries[i..i + 6] {
            for t in &e.tags {
                if !tags.contains(t) {
                    tags.push(t.clone());
                }
            }
        }
        if tags.first().map(String::as_str) != Some("count") {
            return Err(f.error(666, "internal - "));
        }
        for e in &mut result.entries[i..i + 6] {
            e.tags = tags.clone();
        }
        i += 6;
    }
    Ok(result)
}
