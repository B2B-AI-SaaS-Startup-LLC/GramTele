use std::collections::BTreeMap;
use tg_rpl::{EventStream, Producer};
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CallingCodeInfo {
    pub calling_code: String,
    pub prefixes: Vec<String>,
    pub patterns: Vec<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Info {
    pub name: String,
    pub iso2: String,
    pub alternative_name: String,
    pub codes: Vec<CallingCodeInfo>,
    pub is_hidden: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Naming {
    #[default]
    Default,
    Polls,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FormatResult {
    pub formatted: String,
    pub groups: Vec<i32>,
    pub code: String,
}
#[derive(Clone, Debug, Default)]
pub struct FormatArgs {
    pub phone: String,
    pub only_groups: bool,
    pub skip_code: bool,
    pub incomplete: bool,
    pub only_code: bool,
}
pub struct CountriesInstance {
    list: Vec<Info>,
    updated: EventStream<()>,
}
impl Default for CountriesInstance {
    fn default() -> Self {
        Self {
            list: super::fallback::fallback_list(),
            updated: EventStream::new(),
        }
    }
}
impl CountriesInstance {
    pub fn list(&self) -> &[Info] {
        &self.list
    }
    pub fn set_list(&mut self, list: Vec<Info>) {
        self.list = if list.is_empty() {
            super::fallback::fallback_list()
        } else {
            list
        };
        self.updated.fire(());
    }
    pub fn by_code(&self) -> BTreeMap<&str, &Info> {
        let mut result = BTreeMap::new();
        for i in &self.list {
            for c in &i.codes {
                result.insert(c.calling_code.as_str(), i);
            }
        }
        result
    }
    pub fn by_iso2(&self) -> BTreeMap<&str, &Info> {
        self.list.iter().map(|i| (i.iso2.as_str(), i)).collect()
    }
    pub fn valid_phone_code(&self, full: &str) -> String {
        let map = self.by_code();
        let mut full = full.to_owned();
        while !full.is_empty() {
            if map.contains_key(full.as_str()) {
                return full;
            }
            full.pop();
        }
        String::new()
    }
    pub fn country_name_by_iso2(&self, iso: &str, naming: Naming) -> String {
        if naming == Naming::Polls && iso.eq_ignore_ascii_case("FT") {
            return "Fragment".into();
        }
        self.by_iso2()
            .get(iso)
            .map_or_else(String::new, |i| i.name.clone())
    }
    pub fn country_iso2_by_phone(&self, phone: &str) -> String {
        self.by_code()
            .get(self.valid_phone_code(phone).as_str())
            .map_or_else(String::new, |i| i.iso2.clone())
    }
    pub fn flag_emoji_by_iso2(&self, iso: &str) -> String {
        if iso.len() != 2 || !iso.bytes().all(|c| c.is_ascii_uppercase()) {
            String::new()
        } else if iso == "FT" {
            "🏴‍☠️".into()
        } else {
            iso.bytes()
                .map(|c| char::from_u32(0x1f1e6 + u32::from(c - b'A')).unwrap())
                .collect()
        }
    }
    pub fn format(&self, args: FormatArgs) -> FormatResult {
        let phone = args.phone.encode_utf16().collect::<Vec<_>>();
        if phone.is_empty() {
            return FormatResult::default();
        }
        let mut best = None;
        let mut best_len = 0;
        for country in &self.list {
            for c in &country.codes {
                let code = c.calling_code.encode_utf16().collect::<Vec<_>>();
                if phone.starts_with(&code) {
                    for prefix in &c.prefixes {
                        let prefix = prefix.encode_utf16().collect::<Vec<_>>();
                        if code.len() + prefix.len() > best_len
                            && phone[code.len()..].starts_with(&prefix)
                        {
                            best = Some(c);
                            best_len = code.len() + prefix.len();
                        }
                    }
                }
            }
        }
        let Some(c) = best else {
            return FormatResult {
                formatted: args.phone,
                ..Default::default()
            };
        };
        if args.only_code {
            return FormatResult {
                code: c.calling_code.clone(),
                ..Default::default()
            };
        }
        let code_size = c.calling_code.encode_utf16().count();
        if args.only_groups && args.incomplete {
            let initial = if args.skip_code {
                vec![]
            } else {
                vec![code_size as i32]
            };
            let mut best_groups = initial.clone();
            let mut best_size = 0;
            let mut max_matches = -1;
            for pattern in &c.patterns {
                let mut groups = initial.clone();
                let mut group_size = 0;
                let mut spaces = 0;
                let mut matches = 0;
                let mut failed = false;
                for (i, ch) in pattern.encode_utf16().enumerate() {
                    let digit = char::from_u32(ch as u32).is_some_and(|c| c.is_numeric());
                    if digit {
                        if phone.get(i - spaces + code_size) == Some(&ch) {
                            matches += 1;
                        } else {
                            failed = true;
                        }
                    }
                    if char::from_u32(ch as u32).is_some_and(char::is_whitespace) {
                        groups.push(group_size);
                        group_size = 0;
                        spaces += 1;
                    } else {
                        group_size += 1;
                    }
                }
                if matches > max_matches {
                    max_matches = if failed { -1 } else { matches };
                    best_groups = groups;
                    best_size = group_size;
                }
            }
            if best_size != 0 {
                best_groups.push(best_size);
            }
            return FormatResult {
                groups: best_groups,
                ..Default::default()
            };
        }
        let part = &phone[code_size..];
        let mut formatted = part.to_vec();
        let mut groups = vec![];
        let mut max_matches = 0;
        for pattern in &c.patterns {
            let pattern = pattern.encode_utf16().collect::<Vec<_>>();
            let mut result = vec![];
            let mut result_groups = vec![];
            let mut pos = 0;
            let mut failed = false;
            let mut matched = 0;
            let mut size = 0;
            for ch in part {
                while pos < pattern.len()
                    && pattern[pos] != b'X' as u16
                    && !char::from_u32(pattern[pos] as u32).is_some_and(|c| c.is_numeric())
                {
                    if args.only_groups {
                        result_groups.push(size);
                        size = 0;
                    } else {
                        result.push(pattern[pos]);
                    }
                    pos += 1;
                }
                if pos >= pattern.len() || pattern[pos] == b'X' as u16 {
                    pos += 1;
                    if args.only_groups {
                        size += 1;
                    } else {
                        result.push(*ch);
                    }
                } else if *ch == pattern[pos] {
                    matched += 1;
                    pos += 1;
                    if args.only_groups {
                        size += 1;
                    } else {
                        result.push(*ch);
                    }
                } else {
                    failed = true;
                    break;
                }
            }
            if size != 0 {
                result_groups.push(size);
            }
            if !failed && matched >= max_matches {
                max_matches = matched;
                if args.only_groups {
                    groups = result_groups;
                } else {
                    formatted = result;
                }
            }
        }
        if !args.skip_code {
            if args.only_groups {
                groups.insert(0, code_size as i32);
            } else {
                formatted = format!("+{} ", c.calling_code)
                    .encode_utf16()
                    .chain(formatted)
                    .collect();
            }
        }
        FormatResult {
            formatted: if args.only_groups {
                String::new()
            } else {
                String::from_utf16_lossy(&formatted)
            },
            groups,
            code: String::new(),
        }
    }
    pub fn extract_phone_code(&self, phone: &str) -> String {
        self.format(FormatArgs {
            phone: phone.into(),
            only_code: true,
            ..Default::default()
        })
        .code
    }
    pub fn groups(&self, phone: &str) -> Vec<i32> {
        self.format(FormatArgs {
            phone: phone.into(),
            only_groups: true,
            incomplete: true,
            ..Default::default()
        })
        .groups
    }
    pub fn updated(&self) -> Producer<()> {
        self.updated.events()
    }
}
