use crate::common::{Error, Result};
pub type InputId = Vec<u32>;
pub type Category = Vec<InputId>;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DoubleColored {
    pub original: InputId,
    pub same: InputId,
    pub different: InputId,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputData {
    pub colored: Category,
    pub double_colored: Vec<DoubleColored>,
    pub categories: Vec<Category>,
    pub other: Category,
}
impl Default for InputData {
    fn default() -> Self {
        Self {
            colored: Vec::new(),
            double_colored: Vec::new(),
            categories: vec![Vec::new(); 7],
            other: Vec::new(),
        }
    }
}
pub fn input_id_from_string(s: &str) -> InputId {
    let units: Vec<_> = s.encode_utf16().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < units.len() {
        let u = units[i] as u32;
        i += 1;
        if (0xd800..=0xdbff).contains(&u) {
            out.push((u << 16) | units[i] as u32);
            i += 1;
        } else {
            out.push(u);
        }
    }
    out
}
pub fn input_id_to_utf16(id: &[u32]) -> Vec<u16> {
    let mut out = Vec::new();
    for &c in id {
        if c > 0xffff {
            out.push((c >> 16) as u16);
        }
        out.push(c as u16);
    }
    out
}
pub fn bad(message: impl Into<String>) -> Error {
    Error::new("input", 0, 401, format!("Bad data: {}", message.into()))
}
type Part = Vec<Vec<String>>;
pub fn read_data(bytes: &[u8]) -> Result<InputData> {
    let text = String::from_utf8_lossy(bytes);
    let mut remaining = text.as_ref();
    let mut file: Vec<Vec<Part>> = Vec::new();
    loop {
        let end = [remaining.find("--------"), remaining.find("========")]
            .into_iter()
            .flatten()
            .min();
        let section = if let Some(n) = end {
            &remaining[..n]
        } else {
            remaining
        };
        let section = section.replace("\r\n", "\n");
        let parts: Vec<Part> = section
            .split("\n\n")
            .map(|part| {
                part.lines()
                    .map(|line| {
                        line.split(',')
                            .filter_map(|s| {
                                let start = s.find('"')?;
                                let end = s[start + 1..].find('"')?;
                                let s = &s[start + 1..start + 1 + end];
                                (!s.is_empty()).then(|| s.to_owned())
                            })
                            .collect::<Vec<_>>()
                    })
                    .filter(|l| !l.is_empty())
                    .collect::<Part>()
            })
            .filter(|p| !p.is_empty())
            .collect();
        if !parts.is_empty() {
            file.push(parts);
        }
        if let Some(end) = end {
            remaining = &remaining[end + 1..];
        } else {
            break;
        }
    }
    if file.len() < 3 || file[0].len() != 8 || file[1].len() > 8 {
        return Err(bad("Wrong file parts."));
    }
    let mut out = InputData::default();
    let find = |emoji: &str| -> Result<&Vec<String>> {
        file[0]
            .iter()
            .flatten()
            .find(|line| line[0] == emoji)
            .ok_or_else(|| bad(format!("Simple colored emoji not found: {emoji}")))
    };
    for emoji in file[2][0].iter().flatten() {
        let line = find(emoji)?;
        if line.len() != 6 {
            return Err(bad(format!("Wrong simple colored emoji: {emoji}")));
        }
        let id = input_id_from_string(&line[1]);
        if id.len() < 2 {
            return Err(bad(format!("Bad colored emoji: {}", line[1])));
        }
        out.colored.push(id);
    }
    if file[2].len() > 1 {
        for emoji in file[2][1].iter().flatten() {
            let line = find(emoji)?;
            if line.len() != 26 {
                return Err(bad(format!("Wrong double colored emoji: {emoji}")));
            }
            let original = input_id_from_string(&line[0]);
            let same = input_id_from_string(&line[1]);
            let different = input_id_from_string(&line[2]);
            if original.is_empty() || same.len() < 2 || different.len() < 5 {
                return Err(bad(format!(
                    "Bad double colored emoji: {}, {}",
                    line[0], line[2]
                )));
            }
            out.double_colored.push(DoubleColored {
                original,
                same,
                different,
            });
        }
    }
    let mut used = 0;
    for (i, part) in file[0].iter().enumerate() {
        let replacement = file[1].iter().find(|p| p[0][0] == part[0][0]);
        if replacement.is_some() {
            used += 1;
        }
        for emoji in replacement.unwrap_or(part).iter().flatten() {
            out.categories[i.min(6)].push(input_id_from_string(emoji));
        }
    }
    if used != file[1].len() {
        return Err(bad("Could not use some non-colored section replacements!"));
    }
    if file.len() > 3 {
        for emoji in file[3].iter().flatten().flatten() {
            out.other.push(input_id_from_string(emoji));
        }
    }
    Ok(out)
}
