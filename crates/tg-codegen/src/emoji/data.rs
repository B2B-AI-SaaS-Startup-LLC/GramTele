use super::{
    data_old,
    data_read::{bad, input_id_to_utf16, InputData, InputId},
};
use crate::common::Result;
use std::collections::{BTreeMap, BTreeSet};
pub const POSTFIX: u32 = 0xfe0f;
pub const JOINER: u32 = 0x200d;
pub const COLORS: [u32; 5] = [0xd83cdffb, 0xd83cdffc, 0xd83cdffd, 0xd83cdffe, 0xd83cdfff];
pub type Id = Vec<u16>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Emoji {
    pub id: Id,
    pub postfixed: bool,
    pub variated: bool,
    pub colored: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Data {
    pub list: Vec<Emoji>,
    pub map: BTreeMap<Id, usize>,
    pub postfix_required: BTreeSet<usize>,
    pub categories: Vec<Vec<usize>>,
    pub replaces: BTreeMap<String, usize>,
}
pub fn bare(id: &[u32]) -> Id {
    input_id_to_utf16(
        &id.iter()
            .copied()
            .filter(|&c| c != POSTFIX)
            .collect::<Vec<_>>(),
    )
}
fn alias(aliases: &mut BTreeMap<Id, Vec<Id>>, real: Id, old: Id) {
    let list = aliases.entry(real).or_default();
    if !list.contains(&old) {
        list.push(old);
    }
}
fn add(
    data: &mut Data,
    emoji: Emoji,
    aliases: &BTreeMap<Id, Vec<Id>>,
    added: &mut BTreeSet<Id>,
) -> Result<usize> {
    let key = emoji
        .id
        .iter()
        .copied()
        .filter(|&c| c != POSTFIX as u16)
        .collect::<Vec<_>>();
    if key.is_empty() {
        return Err(bad("empty emoji id found."));
    }
    if let Some(&index) = data.map.get(&key) {
        let old = &data.list[index];
        if old.postfixed != emoji.postfixed {
            return Err(bad("same emoji found with different postfixed property."));
        }
        if old.id != emoji.id {
            return Err(bad("same emoji found with different id."));
        }
        return Ok(index);
    }
    let index = data.list.len();
    data.map.insert(key.clone(), index);
    if let Some(list) = aliases.get(&key) {
        added.insert(key.clone());
        for a in list {
            if data.map.insert(a.clone(), index).is_some() {
                return Err(bad("some emoji alias already in the map."));
            }
        }
    }
    if [vec![0x2122], vec![0xa9], vec![0xae]].contains(&key) {
        data.postfix_required.insert(index);
    }
    data.list.push(emoji);
    Ok(index)
}
pub fn prepare(input: &InputData, extra_old: &[InputData]) -> Result<Data> {
    if input.colored.is_empty()
        || input.double_colored.is_empty()
        || extra_old
            .iter()
            .any(|old| old.colored.is_empty() || old.double_colored.is_empty())
    {
        // PrepareData/CheckOldInCurrent return false without logging in C++.
        return Err(crate::common::Error::new("", 0, 0, ""));
    }
    let mut aliases = BTreeMap::new();
    let mut variated = BTreeSet::new();
    let mut double = BTreeMap::new();
    for row in &input.colored {
        if row.len() < 2 {
            return Err(bad("colored string should have at least two characters."));
        }
        if row[1] != COLORS[0] {
            return Err(bad("color code should appear at index 1."));
        }
        if row
            .iter()
            .enumerate()
            .any(|(i, &c)| c == COLORS[0] && i != 1)
        {
            return Err(bad("color code should appear only at index 1."));
        }
        let mut id = row.clone();
        id.remove(1);
        variated.insert(bare(&id));
    }
    for row in &input.double_colored {
        let (o, s, d) = (&row.original, &row.same, &row.different);
        if o.is_empty() {
            return Err(bad("original string should have at least one character."));
        }
        if s.len() < 2 {
            return Err(bad("colored string should have at least two characters."));
        }
        let key;
        if s.len() == 2 {
            if s[1] != COLORS[0] {
                return Err(bad("color code should appear at index 1."));
            }
            if s[0] == COLORS[0] {
                return Err(bad("color code should appear only at index 1."));
            }
            if s[0] == POSTFIX {
                return Err(bad("postfix in double colored is not supported."));
            }
            if d.len() == 5 {
                if d.iter().filter(|&&c| c == JOINER).count() != 1 || d[2] != JOINER {
                    return Err(bad("complex short double colored bad different."));
                }
            } else if d.len() >= 7 {
                if d.iter().filter(|&&c| c == JOINER).count() != 2 {
                    return Err(bad("complex double colored bad different."));
                }
            } else {
                return Err(bad("complex double colored unknown different."));
            }
            for color in COLORS {
                let mut same = s.clone();
                same[1] = color;
                let diff = d
                    .iter()
                    .map(|&c| {
                        if c == COLORS[0] || c == COLORS[1] {
                            color
                        } else {
                            c
                        }
                    })
                    .collect::<Vec<_>>();
                alias(&mut aliases, bare(&same), bare(&diff));
            }
            key = bare(&s[..1]);
        } else {
            if d.len() < 7 || d[1] != COLORS[0] || d[d.len() - 1] != COLORS[1] {
                return Err(bad("complex double colored bad different."));
            }
            let mut copy = d.clone();
            *copy.last_mut().unwrap() = COLORS[0];
            if copy != *s {
                return Err(bad("complex double colored should colorize all the same."));
            }
            if o.len() == 1 {
                for color in COLORS {
                    let mut original = o.clone();
                    original.push(color);
                    let same = s
                        .iter()
                        .map(|&c| if c == COLORS[0] { color } else { c })
                        .collect::<Vec<_>>();
                    alias(&mut aliases, bare(&original), bare(&same));
                }
            }
            key = bare(o);
        }
        double.entry(key).or_insert_with(|| d.clone());
    }
    for old in [data_old::get_data_old1(), data_old::get_data_old2()]
        .iter()
        .chain(extra_old)
    {
        check_old(input, old, &mut aliases)?;
    }
    alias(
        &mut aliases,
        bare(&[0xd83cddeb, 0xd83cddf7]),
        bare(&[0xd83cdde8, 0xd83cddf5]),
    );
    let mut result = Data::default();
    let mut added = BTreeSet::new();
    for category in input.categories.iter().chain(std::iter::once(&input.other)) {
        let mut output = Vec::new();
        for id in category {
            if id.iter().any(|c| COLORS.contains(c)) {
                return Err(bad("color code found in a category emoji."));
            }
            let key = bare(id);
            let postfixed = id.len() == 2 && id[1] == POSTFIX;
            let trimmed = if postfixed { &id[..1] } else { id.as_slice() };
            let index = add(
                &mut result,
                Emoji {
                    id: input_id_to_utf16(trimmed),
                    postfixed,
                    variated: false,
                    colored: false,
                },
                &aliases,
                &mut added,
            )?;
            output.push(index);
            if variated.contains(&key) {
                result.list[index].variated = true;
                if trimmed[0] == POSTFIX {
                    return Err(bad("bad first symbol in emoji."));
                }
                let mut tail = &trimmed[1..];
                if tail.iter().filter(|&&c| c == POSTFIX).count() == 2 && tail[0] == POSTFIX {
                    tail = &tail[1..];
                }
                for color in COLORS {
                    let mut colored = vec![trimmed[0], color];
                    colored.extend_from_slice(tail);
                    add(
                        &mut result,
                        Emoji {
                            id: input_id_to_utf16(&colored),
                            postfixed: false,
                            variated: false,
                            colored: true,
                        },
                        &aliases,
                        &mut added,
                    )?;
                }
            } else if let Some(different) = double.get(&key) {
                if different.len() < 4
                    || different[1] != COLORS[0]
                    || different[different.len() - 1] != COLORS[1]
                {
                    return Err(bad("bad data in double-colored emoji."));
                }
                for color1 in COLORS {
                    for color2 in COLORS {
                        let colored = if color1 == color2 && key.len() == 2 {
                            let mut id = key.clone();
                            id.extend(input_id_to_utf16(&[color1]));
                            id
                        } else {
                            let mut copy = different.clone();
                            copy[1] = color1;
                            *copy.last_mut().unwrap() = color2;
                            input_id_to_utf16(&copy)
                        };
                        // QString::replace mutates colored.id in the original double-color branch.
                        let colored = colored
                            .into_iter()
                            .filter(|&c| c != POSTFIX as u16)
                            .collect();
                        add(
                            &mut result,
                            Emoji {
                                id: colored,
                                postfixed: false,
                                variated: false,
                                colored: false,
                            },
                            &aliases,
                            &mut added,
                        )?;
                    }
                }
            }
        }
        result.categories.push(output);
    }
    if added.len() != aliases.len() {
        let key = aliases.keys().find(|k| !added.contains(*k)).unwrap();
        return Err(bad(format!(
            "Not added aliases list for: {}",
            key.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )));
    }
    for (code, text) in EMOTICONS {
        let index = *result
            .map
            .get(&input_id_to_utf16(&[*code]))
            .ok_or_else(|| bad("emoji from replaces not found in the map."))?;
        result.replaces.entry((*text).into()).or_insert(index);
    }
    Ok(result)
}
fn check_old(data: &InputData, old: &InputData, aliases: &mut BTreeMap<Id, Vec<Id>>) -> Result<()> {
    let find = |id: &InputId| {
        data.categories
            .iter()
            .chain(std::iter::once(&data.other))
            .any(|c| c.contains(id))
    };
    for (c, category) in old.categories.iter().enumerate() {
        for (i, id) in category.iter().enumerate() {
            if find(id) {
                continue;
            }
            let mut modified = id.clone();
            if modified.last() == Some(&POSTFIX) {
                modified.pop();
            } else {
                modified.push(POSTFIX);
            }
            if !find(&modified) {
                return Err(bad(format!(
                    "old emoji '{}' (category {c}, index {i}) not found in current.",
                    String::from_utf16_lossy(&input_id_to_utf16(id))
                )));
            }
        }
    }
    for (i, row) in old.double_colored.iter().enumerate() {
        match data
            .double_colored
            .iter()
            .find(|r| r.original == row.original)
        {
            None => {
                return Err(bad(format!(
                    "old double colored emoji (index {i}) not found in current."
                )))
            }
            Some(r) if r != row => {
                return Err(bad(format!(
                    "old double colored emoji (index {i}) not equal to current."
                )))
            }
            _ => {}
        }
    }
    for (i, id) in old.colored.iter().enumerate() {
        if data.colored.contains(id) {
            continue;
        }
        let other = [0x2640, 0x2642]
            .iter()
            .position(|&gender| {
                let mut v = id.clone();
                v.extend([JOINER, gender, POSTFIX]);
                old.colored.contains(&v)
            })
            .ok_or_else(|| {
                bad(format!(
                    "old colored emoji (index {i}) not found in current."
                ))
            })?;
        let gender = 1 - other;
        let mut real = id.clone();
        real.extend([JOINER, [0x2640, 0x2642][gender], POSTFIX]);
        if !data.colored.contains(&real) {
            return Err(bad(format!(
                "old colored emoji (index {i}) not found in current with added gender: {gender}."
            )));
        }
        if real.len() < 2 || id.len() < 2 || real[1] != COLORS[0] || id[1] != COLORS[0] {
            return Err(bad("bad colored emoji."));
        }
        for color in COLORS {
            let mut key = real.clone();
            key[1] = color;
            let mut value = id.clone();
            value[1] = color;
            alias(aliases, bare(&key), bare(&value));
        }
    }
    Ok(())
}
pub const EMOTICONS: &[(u32, &str)] = &[
    (0xd83dde0a, ":-)"),
    (0xd83dde0d, "8-)"),
    (0x2764, "<3"),
    (0xd83dde1a, ":-*"),
    (0xd83dde0e, "B-)"),
    (0xd83dde03, ":-D"),
    (0xd83dde09, ";-)"),
    (0xd83dde1c, ";-P"),
    (0xd83dde0b, ":-p"),
    (0xd83dde14, "3("),
    (0xd83dde1e, ":-("),
    (0xd83dde0f, ":]"),
    (0xd83dde22, ":'("),
    (0xd83dde2d, ":_("),
    (0xd83dde29, ":(("),
    (0xd83dde10, ":|"),
    (0xd83dde0c, "3-)"),
    (0xd83dde20, ">("),
    (0xd83dde21, ">(("),
    (0xd83dde07, "O:)"),
    (0xd83dde30, ";o"),
    (0xd83dde33, "8|"),
    (0xd83dde32, "8o"),
    (0xd83dde37, ":X"),
    (0xd83dde08, "}:)"),
];
