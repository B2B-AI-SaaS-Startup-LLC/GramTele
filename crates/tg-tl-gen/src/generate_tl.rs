//! Port of the parsing and validation half of `lib_tl/tl/generate_tl.py`
//! (`readInputs` and the first loop of `readAndGenerate`).
//!
//! The Python script parses the scheme with regular expressions; this module
//! reproduces each of them by hand (the comments name the original regex),
//! including the constructor id check (`binascii.crc32(a2b_qp(cleanline))`),
//! the `typeIdExceptions`, `renamedTypes`, `skip`, `synonyms` and
//! `flagInheritance` options, the `flags2` → 64-bit flags merge and the
//! "bots only" / "official mobile applications only" line filter.
//!
//! Differences:
//! * Errors are returned instead of raised; warnings (the script's `print`)
//!   are collected in [`Schema::warnings`].
//! * Parameter types are resolved after the whole scheme is read instead of
//!   at the point of use; for any scheme the original accepts the result is
//!   the same.
//! * Things the original silently turns into broken C++ or a broken wire
//!   format are rejected: a second `#` parameter not named `<flags>2`, a
//!   value parameter between `flags` and `flags2` (the original writes both
//!   words together), "bots only" / nullable parameters (dropped from
//!   reading and writing by the original's read-write mode), bare
//!   multi-constructor vector items.

use std::collections::{HashMap, HashSet};

/// Generator options: the dictionary `codegen_scheme.py` passes to
/// `generate()`, limited to the keys that influence read-write output.
#[derive(Clone, Debug, Default)]
pub struct Config {
    /// `flagInheritance`: `(child, parent)` constructor pairs whose flags can
    /// be cast (`mtpCastFlags`).
    pub flag_inheritance: Vec<(String, String)>,
    /// `typeIdExceptions`: `name#id` definitions accepted although the id
    /// does not match the CRC of the line.
    pub type_id_exceptions: Vec<String>,
    /// `renamedTypes`: `(original, new)` constructor / method names.
    pub renamed_types: Vec<(String, String)>,
    /// `skip`: whole lines (after `strip()`) that are ignored.
    pub skip: Vec<String>,
    /// `builtin`: primitive type names.
    pub builtin: Vec<String>,
    /// `builtinTemplates`.
    pub builtin_templates: Vec<String>,
    /// `synonyms`: `(name, same_as)` used when computing the CRC.
    pub synonyms: Vec<(String, String)>,
}

/// One input file.
#[derive(Clone, Debug)]
pub struct Input {
    /// File name (used in the generated header).
    pub name: String,
    /// File contents.
    pub text: String,
}

/// Primitive (builtin) types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `int`
    Int,
    /// `long`
    Long,
    /// `double`
    Double,
    /// `string`
    String,
    /// `bytes`
    Bytes,
    /// `int128`
    Int128,
    /// `int256`
    Int256,
}

impl Primitive {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "int" => Self::Int,
            "long" => Self::Long,
            "double" => Self::Double,
            "string" => Self::String,
            "bytes" => Self::Bytes,
            "int128" => Self::Int128,
            "int256" => Self::Int256,
            _ => return None,
        })
    }
}

/// A resolved parameter or result type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeRef {
    /// A builtin type, always bare.
    Primitive(Primitive),
    /// A boxed type, by index into [`Schema::types`].
    Boxed(usize),
    /// A constructor used bare (`vector<future_salt>`), by index into
    /// [`Schema::constructors`].
    Bare(usize),
    /// `Vector<T>` (`boxed == true`) or `vector<T>`.
    Vector {
        /// Written with the `vector` constructor id.
        boxed: bool,
        /// Item type.
        item: Box<TypeRef>,
    },
    /// The `{X:Type}` template parameter (`!X`, result `X`).
    Template,
}

/// What a parameter is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParamKind {
    /// The `#` flags word. `wide` if the definition also has `flags2`
    /// (merged into 64 bits).
    Flags {
        /// Two words (`flags` + `flags2`).
        wide: bool,
    },
    /// `flags.N?true`: only a bit, nothing is serialized.
    True {
        /// Bit 0..63 (`flags2` bits are 32..63).
        bit: u32,
    },
    /// A serialized value, conditional on `bit` if present.
    Value {
        /// Value type.
        ty: TypeRef,
        /// Flag bit (0..63) the value depends on.
        bit: Option<u32>,
    },
}

/// One parameter of a definition (`prmsList` entry; `flags2` is merged into
/// the flags parameter and does not appear).
#[derive(Clone, Debug)]
pub struct Param {
    /// Name as in the scheme.
    pub name: String,
    /// Kind.
    pub kind: ParamKind,
}

/// A constructor or a method.
#[derive(Clone, Debug)]
pub struct Definition {
    /// Name as written in the scheme (`messages.getHistory`).
    pub original_name: String,
    /// Name after `renamedTypes`, with the dot (`messages.getHistory`).
    pub name: String,
    /// Constructor id.
    pub id: u32,
    /// Parameters in scheme order.
    pub params: Vec<Param>,
    /// Result type (for a constructor always [`TypeRef::Boxed`]).
    pub result: TypeRef,
    /// The template parameter name (`X`) for generic methods.
    pub template: Option<String>,
    /// `true` for methods.
    pub is_function: bool,
    /// The flags parameter name, if any.
    pub flags: Option<String>,
    /// The merged `flags2` parameter name, if any.
    pub flags64: Option<String>,
}

impl Definition {
    /// `normalizedName(name)`: the suffix of all C++ names of the
    /// definition (`messages_getHistory`).
    pub fn cpp_name(&self) -> String {
        self.name.replace('.', "_")
    }

    /// `len(prms) > len(trivialConditions)`: the definition stores data.
    pub fn has_data(&self) -> bool {
        self.params
            .iter()
            .any(|p| !matches!(p.kind, ParamKind::True { .. }))
    }
}

/// A boxed type (`typesList` entry).
#[derive(Clone, Debug)]
pub struct TypeInfo {
    /// Name as in the scheme (`messages.Messages`).
    pub name: String,
    /// Constructors, by index into [`Schema::constructors`], in scheme order.
    pub constructors: Vec<usize>,
}

impl TypeInfo {
    /// `normalizedName(resType)`: suffix of the boxed C++ name
    /// (`MTPmessages_Messages`).
    pub fn cpp_name(&self) -> String {
        self.name.replace('.', "_")
    }

    /// `restype`: suffix of the bare C++ class (`MTPmessages_messages`).
    pub fn cpp_bare_name(&self) -> String {
        match self.name.rfind('.') {
            Some(i) => format!(
                "{}_{}",
                self.name[..i].replace('.', "_"),
                lower_first(&self.name[i + 1..])
            ),
            None => lower_first(&self.name),
        }
    }
}

/// A parsed scheme.
#[derive(Clone, Debug, Default)]
pub struct Schema {
    /// `// LAYER N`.
    pub layer: u32,
    /// Input file names.
    pub input_names: Vec<String>,
    /// All generated constructors in scheme order.
    pub constructors: Vec<Definition>,
    /// All generated methods in scheme order.
    pub functions: Vec<Definition>,
    /// Boxed types in order of first constructor.
    pub types: Vec<TypeInfo>,
    /// `(child, parent)` constructor index pairs for `mtpCastFlags`.
    pub flag_casts: Vec<(usize, usize)>,
    /// Definitions skipped for a constructor id mismatch (the original's
    /// "Warning: counted ... mismatch" + `continue`) and other warnings.
    pub warnings: Vec<String>,
    /// Lines skipped as "bots only" / "official mobile applications only".
    pub skipped_lines: Vec<String>,
    /// Number of definition lines seen (before any filtering, after `skip`).
    pub definition_lines: usize,
}

impl Schema {
    /// Index of a boxed type by scheme name.
    pub fn type_index(&self, name: &str) -> Option<usize> {
        self.types.iter().position(|t| t.name == name)
    }

    /// Index of a constructor by scheme name.
    pub fn constructor_index(&self, name: &str) -> Option<usize> {
        self.constructors.iter().position(|c| c.name == name)
    }
}

fn lower_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn upper_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

// --- comment-based filters (endsWithForTag & co) ---

fn ends_with_for_tag(comments: &str, tag: &str, ending: &str) -> bool {
    let needle = format!("@{tag} ");
    let Some(position) = comments.find(&needle) else {
        return false;
    };
    let tail = &comments[position + tag.len() + 1..];
    let line = match tail.find('@') {
        Some(till) => &tail[..till],
        None => tail,
    };
    let stripped = line.trim();
    let full = format!("; {}", ending.trim());
    if stripped.len() < full.len() {
        return false;
    }
    if stripped.ends_with(&full) {
        return true;
    }
    [
        ".",
        ";",
        " if",
        " to",
        " otherwise",
        " unless",
        " even",
        " for",
    ]
    .iter()
    .any(|suffix| stripped.contains(&format!("{full}{suffix}")))
}

fn param_name_tag(name: &str) -> &str {
    if name == "description" {
        "param_description"
    } else {
        name
    }
}

fn is_bots_only_line(comments: &str) -> bool {
    ends_with_for_tag(comments, "description", "for bots only")
        || ends_with_for_tag(comments, "description", "bots only")
}

fn is_bots_only_param(comments: &str, name: &str) -> bool {
    let tag = param_name_tag(name);
    ends_with_for_tag(comments, tag, "for bots only")
        || ends_with_for_tag(comments, tag, "bots only")
}

fn is_mobile_only_line(comments: &str) -> bool {
    ends_with_for_tag(
        comments,
        "description",
        "for official mobile applications only",
    )
}

fn is_nullable_vector(comments: &str, name: &str) -> bool {
    name.ends_with('s')
        && ends_with_for_tag(
            comments,
            param_name_tag(name),
            &format!("{name} may be null"),
        )
}

fn is_nullable_param(comments: &str, name: &str) -> bool {
    let tag = param_name_tag(name);
    ends_with_for_tag(comments, tag, "may be null") || ends_with_for_tag(comments, tag, "pass null")
}

// --- constructor id ---

/// `binascii.a2b_qp(data)` (header = False).
pub fn a2b_qp(data: &[u8]) -> Vec<u8> {
    let hex = |c: u8| -> Option<u8> { (c as char).to_digit(16).map(|d| d as u8) };
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'=' {
            i += 1;
            if i >= data.len() {
                break;
            }
            if data[i] == b'\n' || data[i] == b'\r' {
                if data[i] != b'\n' {
                    while i < data.len() && data[i] != b'\n' {
                        i += 1;
                    }
                }
                if i < data.len() {
                    i += 1;
                }
            } else if data[i] == b'=' {
                out.push(b'=');
                i += 1;
            } else if let (Some(h), Some(l)) = (hex(data[i]), data.get(i + 1).and_then(|&c| hex(c)))
            {
                out.push((h << 4) | l);
                i += 2;
            } else {
                out.push(b'=');
            }
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    out
}

fn is_ident_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// `re.sub(r' [a-zA-Z0-9_]+\:flags2?\.[0-9]+\?true', '', s)`.
fn remove_true_flags(s: &str) -> String {
    let b = s.as_bytes();
    let try_match = |start: usize| -> Option<usize> {
        let mut i = start;
        if b.get(i) != Some(&b' ') {
            return None;
        }
        i += 1;
        let name_start = i;
        while i < b.len() && is_ident_char(b[i]) {
            i += 1;
        }
        if i == name_start || b.get(i) != Some(&b':') {
            return None;
        }
        i += 1;
        if !b[i..].starts_with(b"flags") {
            return None;
        }
        i += 5;
        if b.get(i) == Some(&b'2') && b.get(i + 1) == Some(&b'.') {
            i += 1;
        }
        if b.get(i) != Some(&b'.') {
            return None;
        }
        i += 1;
        let digits = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == digits || !b[i..].starts_with(b"?true") {
            return None;
        }
        Some(i + 5)
    };
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if let Some(end) = try_match(i) {
            i = end;
        } else {
            // All characters handled here are ASCII in a valid scheme; copy
            // whole UTF-8 sequences anyway.
            let c = s[i..].chars().next().unwrap_or(' ');
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// The `cleanline` the original feeds to the CRC, from the three regex
/// groups (name, params, result type).
pub fn clean_line(name: &str, params: &str, result: &str, synonyms: &[(String, String)]) -> String {
    let mut cleanline = format!("{name}{params}= {result}");
    cleanline = remove_true_flags(&cleanline);
    cleanline = cleanline.replace(['<', '>'], " ").replace("  ", " ");
    if let Some(rest) = cleanline.strip_prefix(' ') {
        cleanline = rest.to_string();
    }
    if let Some(rest) = cleanline.strip_suffix(' ') {
        cleanline = rest.to_string();
    }
    for (synonym, of) in synonyms {
        cleanline = cleanline.replace(&format!(":{synonym} "), &format!(":{of} "));
        cleanline = cleanline.replace(&format!("?{synonym} "), &format!("?{of} "));
    }
    cleanline.replace(['{', '}'], "")
}

/// `binascii.crc32(binascii.a2b_qp(cleanline))`.
pub fn line_crc(cleanline: &str) -> u32 {
    crc32fast::hash(&a2b_qp(cleanline.as_bytes()))
}

// --- line parsing ---

struct NameType<'a> {
    name: &'a str,
    id: Option<&'a str>,
    params: &'a str,
    result: &'a str,
}

fn is_name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'.' || c == b'_'
}

fn is_type_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'.' | b'<' | b'>' | b'_')
}

/// `re.match(r'([a-zA-Z\.0-9_]+)(#[0-9a-f]+)?([^=]*)=\s*([a-zA-Z\.<>0-9_]+);', line)`.
fn match_name_type(line: &str) -> Option<NameType<'_>> {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() && is_name_char(b[i]) {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    let name = &line[..i];
    let mut id = None;
    if b.get(i) == Some(&b'#') {
        let start = i + 1;
        let mut j = start;
        while j < b.len() && matches!(b[j], b'0'..=b'9' | b'a'..=b'f') {
            j += 1;
        }
        if j > start {
            id = Some(&line[start..j]);
            i = j;
        }
    }
    let params_start = i;
    while i < b.len() && b[i] != b'=' {
        i += 1;
    }
    if i >= b.len() {
        return None;
    }
    let params = &line[params_start..i];
    i += 1;
    while i < b.len() && (b[i] as char).is_ascii_whitespace() {
        i += 1;
    }
    let result_start = i;
    while i < b.len() && is_type_char(b[i]) {
        i += 1;
    }
    if i == result_start || b.get(i) != Some(&b';') {
        return None;
    }
    Some(NameType {
        name,
        id,
        params,
        result: &line[result_start..i],
    })
}

enum RawParamType<'a> {
    Plain(&'a str),
    Template(&'a str),
    Flags,
    Conditional {
        flags: &'a str,
        bit: u32,
        ty: &'a str,
    },
}

/// `re.match(r'([a-zA-Z_][a-zA-Z0-9_]*):([A-Za-z0-9<>\._]+|![a-zA-Z]+|\#|[a-z_][a-z0-9_]*\.[0-9]+\?[A-Za-z0-9<>\._]+)$', param)`.
fn match_param(param: &str) -> Option<(&str, RawParamType<'_>)> {
    let colon = param.find(':')?;
    let name = &param[..colon];
    let nb = name.as_bytes();
    if nb.is_empty()
        || !(nb[0].is_ascii_alphabetic() || nb[0] == b'_')
        || !nb.iter().all(|&c| is_ident_char(c))
    {
        return None;
    }
    let ty = &param[colon + 1..];
    let tb = ty.as_bytes();
    if ty == "#" {
        return Some((name, RawParamType::Flags));
    }
    if let Some(rest) = ty.strip_prefix('!') {
        if !rest.is_empty() && rest.bytes().all(|c| c.is_ascii_alphabetic()) {
            return Some((name, RawParamType::Template(rest)));
        }
        return None;
    }
    if !tb.is_empty() && tb.iter().all(|&c| is_type_char(c)) {
        return Some((name, RawParamType::Plain(ty)));
    }
    let (flags, rest) = ty.split_once('.')?;
    let fb = flags.as_bytes();
    if fb.is_empty()
        || !(fb[0].is_ascii_lowercase() || fb[0] == b'_')
        || !fb
            .iter()
            .all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
    {
        return None;
    }
    let (bit, inner) = rest.split_once('?')?;
    if bit.is_empty() || !bit.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if inner.is_empty() || !inner.bytes().all(is_type_char) {
        return None;
    }
    Some((
        name,
        RawParamType::Conditional {
            flags,
            bit: bit.parse().ok()?,
            ty: inner,
        },
    ))
}

enum RawKind {
    Flags,
    True { bit: u32 },
    Value { ty: String, bit: Option<u32> },
    Template,
}

struct RawDefinition {
    original_name: String,
    name: String,
    id: u32,
    params: Vec<(String, RawKind)>,
    result: String,
    template: Option<String>,
    is_function: bool,
    flags: Option<String>,
    flags64: Option<String>,
    line: String,
}

/// Parses and validates the scheme files.
pub fn parse(inputs: &[Input], config: &Config) -> Result<Schema, String> {
    let mut schema = Schema::default();

    // readInputs
    let mut lines: Vec<String> = Vec::new();
    for input in inputs {
        schema.input_names.push(input.name.clone());
        lines.push("---types---".to_string());
        for line in input.text.split_inclusive('\n') {
            if let Some(rest) = line.strip_prefix("// LAYER ") {
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !digits.is_empty() {
                    schema.layer = digits.parse().map_err(|e| format!("Bad layer: {e}"))?;
                    continue;
                }
            }
            lines.push(line.to_string());
        }
    }

    let renamed: HashMap<&str, &str> = config
        .renamed_types
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let exceptions: HashSet<&str> = config
        .type_id_exceptions
        .iter()
        .map(String::as_str)
        .collect();
    let skip: HashSet<&str> = config.skip.iter().map(String::as_str).collect();

    let mut raw = Vec::new();
    let mut funcs_now = false;
    let mut accumulated_comments = String::new();
    for full_line in &lines {
        // nocomment = re.match(r'^(.*?)//(.*?)$', line)
        let (line, comment) = match full_line.find("//") {
            Some(i) => (
                &full_line[..i],
                Some(full_line[i + 2..].trim_end_matches('\n')),
            ),
            None => (full_line.as_str(), None),
        };
        if line.starts_with("---functions---") {
            funcs_now = true;
            continue;
        }
        if line.starts_with("---types---") {
            funcs_now = false;
            continue;
        }
        if line.trim().is_empty() {
            match comment {
                None => accumulated_comments.clear(),
                Some(c) if !c.is_empty() => {
                    accumulated_comments.push(' ');
                    accumulated_comments.push_str(c);
                }
                Some(_) => {}
            }
            continue;
        }
        if skip.contains(line.trim()) {
            continue;
        }
        schema.definition_lines += 1;
        let nt = match_name_type(line).ok_or_else(|| format!("Bad line found: {line}"))?;
        let comments = std::mem::take(&mut accumulated_comments);
        if is_bots_only_line(&comments) || is_mobile_only_line(&comments) {
            schema.skipped_lines.push(line.trim().to_string());
            continue;
        }

        let original_name = nt.name.to_string();
        let name = renamed
            .get(nt.name)
            .map(|s| s.to_string())
            .unwrap_or_else(|| original_name.clone());
        let given = nt
            .id
            .map(|id| id.trim_start_matches('0'))
            .filter(|id| !id.is_empty());
        let cleanline = clean_line(nt.name, nt.params, nt.result, &config.synonyms);
        let counted = line_crc(&cleanline);
        let id = match given {
            Some(given) => {
                let value = u32::from_str_radix(given, 16)
                    .map_err(|_| format!("Bad type id in line: {line}"))?;
                if value != counted {
                    let key = format!("{original_name}#{given}");
                    if !exceptions.contains(key.as_str()) {
                        schema.warnings.push(format!(
                            "Warning: counted {counted:x} mismatch with provided {given} ({key}, {cleanline})"
                        ));
                        continue;
                    }
                }
                value
            }
            None => counted,
        };

        let mut params = Vec::new();
        let mut template = None;
        let mut is_template_param = None;
        let mut flags: Option<String> = None;
        let mut flags64: Option<String> = None;
        let mut seen_conditions = HashSet::new();
        for param in nt.params.trim().split(' ') {
            if param.trim().is_empty() {
                continue;
            }
            // templ = re.match(r'^{([A-Za-z]+):Type}$', param)
            if let Some(inner) = param
                .strip_prefix('{')
                .and_then(|p| p.strip_suffix(":Type}"))
            {
                if !inner.is_empty() && inner.bytes().all(|c| c.is_ascii_alphabetic()) {
                    template = Some(inner.to_string());
                    continue;
                }
            }
            let (pname, ptype) = match_param(param)
                .ok_or_else(|| format!("Bad param found: \"{param}\" in line: {line}"))?;
            if is_bots_only_param(&comments, pname) {
                return Err(format!(
                    "Bots-only param \"{pname}\" is not supported in read-write mode, line: {line}"
                ));
            }
            if is_nullable_vector(&comments, pname) || is_nullable_param(&comments, pname) {
                return Err(format!(
                    "Nullable param \"{pname}\" is not supported in read-write mode, line: {line}"
                ));
            }
            let kind = match ptype {
                RawParamType::Template(t) => {
                    if template.as_deref() != Some(t) {
                        return Err(format!(
                            "Bad template param name: \"{param}\" in line: {line}"
                        ));
                    }
                    is_template_param = Some(pname.to_string());
                    RawKind::Template
                }
                RawParamType::Flags => {
                    if let Some(f) = &flags {
                        if pname == format!("{f}2") {
                            flags64 = Some(pname.to_string());
                            continue;
                        }
                        return Err(format!("Second flags param \"{pname}\" in line: {line}"));
                    }
                    flags = Some(pname.to_string());
                    RawKind::Flags
                }
                RawParamType::Plain(t) => RawKind::Value {
                    ty: t.to_string(),
                    bit: None,
                },
                RawParamType::Conditional {
                    flags: flags_name,
                    bit,
                    ty,
                } => {
                    if bit > 31 {
                        return Err(format!("Bad flag bit in \"{param}\" in line: {line}"));
                    }
                    let bit = if Some(flags_name) == flags.as_deref() {
                        bit
                    } else if Some(flags_name) == flags64.as_deref() {
                        bit + 32
                    } else {
                        return Err(format!("Bad param found: \"{param}\" in line: {line}"));
                    };
                    if !seen_conditions.insert(pname.to_string()) {
                        return Err(format!("Duplicate param \"{pname}\" in line: {line}"));
                    }
                    if ty == "true" {
                        RawKind::True { bit }
                    } else {
                        RawKind::Value {
                            ty: ty.to_string(),
                            bit: Some(bit),
                        }
                    }
                }
            };
            // flags + flags2 are written as two consecutive words at the
            // position of `flags`, so nothing serialized may sit between.
            if flags.is_some()
                && flags64.is_none()
                && !matches!(kind, RawKind::True { .. } | RawKind::Flags)
            {
                let declares_flags2 = nt
                    .params
                    .split(' ')
                    .any(|p| flags.as_ref().is_some_and(|f| p == format!("{f}2:#")));
                if declares_flags2 {
                    return Err(format!(
                        "Serialized param \"{pname}\" between flags and flags2 in line: {line}"
                    ));
                }
            }
            params.push((pname.to_string(), kind));
        }
        if template.is_some() != is_template_param.is_some() && funcs_now {
            return Err(format!("Template without template param in line: {line}"));
        }
        if template.is_none() && nt.result == "X" {
            return Err(format!(
                "Bad response type \"X\" in \"{name}\" in line: {line}"
            ));
        }
        if !funcs_now && template.is_some() {
            schema.warnings.push(format!(
                "Template types not allowed: \"{}\" in line: {line}",
                nt.result
            ));
            continue;
        }
        raw.push(RawDefinition {
            original_name,
            name,
            id,
            params,
            result: nt.result.to_string(),
            template,
            is_function: funcs_now,
            flags,
            flags64,
            line: line.trim().to_string(),
        });
    }

    resolve(schema, raw, config)
}

fn is_boxed_name(name: &str) -> bool {
    // re.match(r'^[A-Z]', t) or re.match(r'^[a-zA-Z0-9]+\.[A-Z]', t)
    let b = name.as_bytes();
    if b.first().is_some_and(|c| c.is_ascii_uppercase()) {
        return true;
    }
    match name.find('.') {
        Some(i) if i > 0 => {
            name[..i].bytes().all(|c| c.is_ascii_alphanumeric())
                && b.get(i + 1).is_some_and(|c| c.is_ascii_uppercase())
        }
        _ => false,
    }
}

fn resolve(mut schema: Schema, raw: Vec<RawDefinition>, config: &Config) -> Result<Schema, String> {
    // Boxed types in order of first constructor (typesList).
    let mut type_by_name: HashMap<String, usize> = HashMap::new();
    let mut ctor_by_cpp: HashMap<String, usize> = HashMap::new();
    for (ctor_count, def) in raw.iter().filter(|d| !d.is_function).enumerate() {
        if def.result.contains('<') || !is_boxed_name(&def.result) {
            return Err(format!(
                "Bad result type name: {} in line: {}",
                def.result, def.line
            ));
        }
        let index = *type_by_name.entry(def.result.clone()).or_insert_with(|| {
            schema.types.push(TypeInfo {
                name: def.result.clone(),
                constructors: Vec::new(),
            });
            schema.types.len() - 1
        });
        schema.types[index].constructors.push(ctor_count);
        ctor_by_cpp.insert(def.name.replace('.', "_"), ctor_count);
    }
    let builtins: HashSet<&str> = config.builtin.iter().map(String::as_str).collect();

    let resolve_type = |ty: &str, template: Option<&str>, line: &str| -> Result<TypeRef, String> {
        fn inner(
            ty: &str,
            template: Option<&str>,
            line: &str,
            types: &HashMap<String, usize>,
            ctors: &HashMap<String, usize>,
            builtins: &HashSet<&str>,
        ) -> Result<TypeRef, String> {
            // handleTemplate: r'^([vV]ector<)([A-Za-z0-9\._<>]+)>$'
            for (prefix, boxed) in [("Vector<", true), ("vector<", false)] {
                if let Some(rest) = ty.strip_prefix(prefix) {
                    let item = rest
                        .strip_suffix('>')
                        .ok_or_else(|| format!("Bad template type: {ty} in line: {line}"))?;
                    let item = if item.contains('<') {
                        inner(item, template, line, types, ctors, builtins)?
                    } else if is_boxed_name(item) {
                        TypeRef::Boxed(
                            *types
                                .get(item)
                                .ok_or_else(|| format!("Unknown type {item} in line: {line}"))?,
                        )
                    } else if builtins.contains(item) {
                        TypeRef::Primitive(
                            Primitive::from_name(item)
                                .ok_or_else(|| format!("Unknown builtin {item}"))?,
                        )
                    } else if let Some(&c) = ctors.get(&item.replace('.', "_")) {
                        TypeRef::Bare(c)
                    } else {
                        return Err(format!("Bad vector param: {item} in line: {line}"));
                    };
                    return Ok(TypeRef::Vector {
                        boxed,
                        item: Box::new(item),
                    });
                }
            }
            if ty.contains('<') {
                return Err(format!("Bad template type: {ty} in line: {line}"));
            }
            if Some(ty) == template {
                return Ok(TypeRef::Template);
            }
            if let Some(&c) = ctors.get(&ty.replace('.', "_")) {
                return Ok(TypeRef::Bare(c));
            }
            if builtins.contains(ty) {
                if let Some(p) = Primitive::from_name(ty) {
                    return Ok(TypeRef::Primitive(p));
                }
            }
            types
                .get(ty)
                .map(|&t| TypeRef::Boxed(t))
                .ok_or_else(|| format!("Unknown type \"{ty}\" in line: {line}"))
        }
        inner(ty, template, line, &type_by_name, &ctor_by_cpp, &builtins)
    };

    for def in raw {
        let template = def.template.as_deref();
        let mut params = Vec::new();
        for (name, kind) in &def.params {
            let kind = match kind {
                RawKind::Flags => ParamKind::Flags {
                    wide: def.flags64.is_some(),
                },
                RawKind::True { bit } => ParamKind::True { bit: *bit },
                RawKind::Template => ParamKind::Value {
                    ty: TypeRef::Template,
                    bit: None,
                },
                RawKind::Value { ty, bit } => ParamKind::Value {
                    ty: resolve_type(ty, template, &def.line)?,
                    bit: *bit,
                },
            };
            params.push(Param {
                name: name.clone(),
                kind,
            });
        }
        let result = if def.is_function {
            if def.result.contains('<') {
                resolve_type(&def.result, None, &def.line)?
            } else if Some(def.result.as_str()) == template {
                TypeRef::Template
            } else if is_boxed_name(&def.result) {
                TypeRef::Boxed(*type_by_name.get(&def.result).ok_or_else(|| {
                    format!("Unknown result type {} in line: {}", def.result, def.line)
                })?)
            } else {
                return Err(format!(
                    "Bad result type name: {} in line: {}",
                    def.result, def.line
                ));
            }
        } else {
            TypeRef::Boxed(type_by_name[&def.result])
        };
        let resolved = Definition {
            original_name: def.original_name,
            name: def.name,
            id: def.id,
            params,
            result,
            template: def.template,
            is_function: def.is_function,
            flags: def.flags,
            flags64: def.flags64,
        };
        if resolved.is_function {
            schema.functions.push(resolved);
        } else {
            schema.constructors.push(resolved);
        }
    }

    check_bare_items(&schema)?;
    check_flag_inheritance(&mut schema, config)?;
    Ok(schema)
}

/// The original refuses ("Complex bare type found") to dump a bare type with
/// several constructors and cannot read one; reject it up front.
fn check_bare_items(schema: &Schema) -> Result<(), String> {
    fn walk(schema: &Schema, ty: &TypeRef) -> Result<(), String> {
        match ty {
            TypeRef::Bare(c) => {
                let def = &schema.constructors[*c];
                let TypeRef::Boxed(t) = def.result else {
                    unreachable!("constructor result is boxed")
                };
                if schema.types[t].constructors.len() > 1 {
                    return Err(format!("Complex bare type found: \"{}\"", def.name));
                }
                Ok(())
            }
            TypeRef::Vector { item, .. } => walk(schema, item),
            _ => Ok(()),
        }
    }
    for def in schema.constructors.iter().chain(&schema.functions) {
        for p in &def.params {
            if let ParamKind::Value { ty, .. } = &p.kind {
                walk(schema, ty)?;
            }
        }
    }
    Ok(())
}

/// The `flagInheritance` loop at the end of `readAndGenerate`.
fn check_flag_inheritance(schema: &mut Schema, config: &Config) -> Result<(), String> {
    // parentFlagsCheck: data name -> (flag name -> bit)
    let mut check: HashMap<String, Vec<(String, u32)>> = HashMap::new();
    for def in &schema.constructors {
        if def.flags.is_some() {
            let flags = def
                .params
                .iter()
                .filter_map(|p| match p.kind {
                    ParamKind::True { bit } | ParamKind::Value { bit: Some(bit), .. } => {
                        Some((p.name.clone(), bit))
                    }
                    _ => None,
                })
                .collect();
            check.insert(def.cpp_name(), flags);
        }
    }
    for (child, parent) in &config.flag_inheritance {
        if child == "channelForbidden" {
            continue;
        }
        let child_flags = check
            .get(child)
            .cloned()
            .ok_or_else(|| format!("flagInheritance: no flags in {child}"))?;
        let parent_flags = check
            .get_mut(parent)
            .ok_or_else(|| format!("flagInheritance: no flags in {parent}"))?;
        for (flag, bit) in child_flags {
            match parent_flags.iter().find(|(name, _)| *name == flag) {
                Some((_, parent_bit)) if *parent_bit != bit => {
                    return Err(format!(
                        "Flag {flag} has different value in MTPD{parent} which should be a flags-parent of MTPD{child}"
                    ));
                }
                Some(_) => {}
                None => parent_flags.push((flag, bit)),
            }
        }
        let c = schema
            .constructors
            .iter()
            .position(|d| d.cpp_name() == *child)
            .ok_or_else(|| format!("flagInheritance: unknown {child}"))?;
        let p = schema
            .constructors
            .iter()
            .position(|d| d.cpp_name() == *parent)
            .ok_or_else(|| format!("flagInheritance: unknown {parent}"))?;
        schema.flag_casts.push((c, p));
    }
    Ok(())
}

/// `Name`: the boxed C++ suffix of a method (`messages_GetHistory`).
pub fn function_boxed_cpp_name(name: &str) -> String {
    match name.rfind('.') {
        Some(i) => format!("{}_{}", &name[..i], upper_first(&name[i + 1..])).replace('.', "_"),
        None => upper_first(name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qp_decoding() {
        assert_eq!(a2b_qp(b"a = B"), b"a = B");
        assert_eq!(a2b_qp(b"=41=3d"), b"A=");
        assert_eq!(a2b_qp(b"x==y"), b"x=y");
        assert_eq!(a2b_qp(b"x=\ny"), b"xy");
        assert_eq!(a2b_qp(b"x="), b"x");
        assert_eq!(a2b_qp(b"x=4"), b"x=4");
    }

    #[test]
    fn clean_line_rules() {
        let synonyms = vec![("bytes".to_string(), "string".to_string())];
        assert_eq!(
            clean_line(
                "inputMediaPhoto",
                " flags:# spoiler:flags.1?true id:InputPhoto ttl_seconds:flags.0?int ",
                "InputMedia",
                &synonyms
            ),
            "inputMediaPhoto flags:# id:InputPhoto ttl_seconds:flags.0?int = InputMedia"
        );
        assert_eq!(
            clean_line("a", " x:bytes y:Vector<bytes> ", "A", &synonyms),
            "a x:string y:Vector bytes = A"
        );
        assert_eq!(
            clean_line("u", " flags2:# x:flags2.3?true ", "U", &synonyms),
            "u flags2:# = U"
        );
    }

    #[test]
    fn known_ids() {
        // Values from the scheme.
        assert_eq!(line_crc("boolFalse = Bool"), 0xbc799737);
        assert_eq!(line_crc("inputPeerSelf = InputPeer"), 0x7da07ec9);
        assert_eq!(
            line_crc("inputPeerUser user_id:long access_hash:long = InputPeer"),
            0xdde8a54c
        );
    }

    #[test]
    fn name_type_regex() {
        let m = match_name_type("user#abc flags:# id:long = User; // x").unwrap();
        assert_eq!(m.name, "user");
        assert_eq!(m.id, Some("abc"));
        assert_eq!(m.params, " flags:# id:long ");
        assert_eq!(m.result, "User");
        assert!(match_name_type(" user = User;").is_none());
        assert!(match_name_type("user = User").is_none());
        let m = match_name_type("tlsBlockZero length:int = TlsBlock;").unwrap();
        assert_eq!(m.id, None);
    }

    #[test]
    fn param_regex() {
        assert!(matches!(
            match_param("x:flags.3?Vector<int>"),
            Some(("x", RawParamType::Conditional { bit: 3, .. }))
        ));
        assert!(matches!(
            match_param("flags:#"),
            Some(("flags", RawParamType::Flags))
        ));
        assert!(matches!(
            match_param("query:!X"),
            Some(("query", RawParamType::Template("X")))
        ));
        assert!(match_param("x:flags.3?").is_none());
        assert!(match_param("1x:int").is_none());
        assert!(match_param("x:Flags.3?int").is_none());
    }

    #[test]
    fn comment_filters() {
        assert!(is_bots_only_line(" @description Sets x; for bots only"));
        assert!(!is_bots_only_line(" @description Sets x"));
        assert!(is_mobile_only_line(
            " @description Does y; for official mobile applications only. @param x"
        ));
    }
}
