//! Data captured by running the unchanged C++ generators (Qt 6.11.2).
use serde_json::{json, Value};
use std::path::Path;
fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/Telegram")
}
fn reference() -> Value {
    serde_json::from_str(include_str!("fixtures/reference/data.json")).unwrap()
}
fn numeric(text: &str, name: &str) -> Vec<u64> {
    let text = text
        .split_once(&format!(" {name}:"))
        .unwrap()
        .1
        .split_once("= &")
        .unwrap()
        .1
        .split_once(';')
        .unwrap()
        .0
        .replace("true", "1")
        .replace("false", "0");
    text.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect()
}
#[test]
fn original_palette_every_color_index_fallback_and_checksum() {
    let root = root();
    let m = tg_codegen::style::Parser::new(vec![root.join("lib_ui")])
        .parse(&root.join("lib_ui/ui/colors.palette"))
        .unwrap();
    let p = tg_codegen::style::generator::palette(&m).unwrap();
    let original = &reference()["palette"];
    assert_eq!(json!(p.checksum), original["checksum"]);
    let colors: Vec<_> = p
        .colors
        .iter()
        .map(|c| json!([c.name, c.index, c.fallback, c.rgba[0], c.rgba[1], c.rgba[2], c.rgba[3]]))
        .collect();
    assert_eq!(json!(colors), original["colors"]);
}
#[test]
fn original_lang_all_utf16_values_offsets_tag_and_key_order() {
    let p = tg_codegen::lang::parse(
        &std::fs::read(root().join("Resources/langs/lang.strings")).unwrap(),
        "lang.strings",
        &[],
    )
    .unwrap();
    let g = tg_codegen::lang::generate(&p, "", "lang_auto").unwrap();
    assert_eq!(g.tags, include_str!("fixtures/reference/lang_auto.tags"));
    assert_eq!(
        g.indices,
        include_str!("fixtures/reference/lang_auto.indices")
    );
    let mut offsets = vec![0];
    let mut data = Vec::new();
    for e in &p.entries {
        data.extend_from_slice(&e.value);
        offsets.push(data.len());
    }
    let original = &reference()["lang"];
    assert_eq!(json!(data), original["DefaultData"]);
    assert_eq!(json!(offsets), original["Offsets"]);
}
#[test]
fn original_emoji_ids_flags_variation_parents_sections_and_suggestions() {
    let root = root();
    let data = tg_codegen::emoji::data::prepare(
        &tg_codegen::emoji::data_read::read_data(
            &std::fs::read(root.join("lib_ui/emoji.txt")).unwrap(),
        )
        .unwrap(),
        &[],
    )
    .unwrap();
    let replaces = tg_codegen::emoji::replaces::prepare(
        &std::fs::read(root.join("lib_ui/emoji_suggestions/emoji_autocomplete.json")).unwrap(),
        "suggestions",
    )
    .unwrap();
    let replaces =
        tg_codegen::emoji::replaces::check_and_convert(replaces, &data, "suggestions").unwrap();
    let rust = tg_codegen::emoji::generator::generate(&data, &replaces).unwrap();
    let reference = reference();
    let ids: Vec<_> = data
        .list
        .iter()
        .flat_map(|e| e.id.iter().copied())
        .collect();
    assert_eq!(json!(ids), reference["emoji"]["IdData"]);
    assert_eq!(
        json!(numeric(&rust, "EMOJI_INIT")),
        reference["emoji"]["Data"]
    );
    let sections: Vec<_> = data.categories.iter().flatten().copied().collect();
    assert_eq!(json!(sections), reference["emoji"]["SectionData"]);
    for (rust_name, cpp_name) in [
        ("REPLACEMENT_DATA", "ReplacementData"),
        ("WORD_LENGTHS", "ReplacementWordLengths"),
        ("REPLACEMENT_INIT", "ReplacementInitData"),
    ] {
        assert_eq!(
            json!(numeric(&rust, rust_name)),
            reference["suggestions"][cpp_name],
            "{cpp_name}"
        );
    }
    let checksums: Vec<_> = replaces
        .iter()
        .map(|r| tg_codegen::emoji::replaces::count_checksum(&r.replacement))
        .collect();
    assert_eq!(
        json!(checksums),
        reference["suggestions"]["ReplacementChecksums"]
    );
    // BY_CHAR retains grouped indices and the corresponding (UTF-16 char,count) table.
    let mut by_char = std::collections::BTreeMap::<u16, Vec<usize>>::new();
    for (i, r) in replaces.iter().enumerate() {
        for word in &r.words {
            let ch = word.encode_utf16().next().unwrap();
            let list = by_char.entry(ch).or_default();
            if list.last() != Some(&i) {
                list.push(i);
            }
        }
    }
    let indices: Vec<_> = by_char.values().flatten().copied().collect();
    let init: Vec<_> = by_char
        .iter()
        .flat_map(|(&ch, v)| [ch as usize, v.len()])
        .collect();
    assert_eq!(
        json!(indices),
        reference["suggestions"]["ReplacementIndices"]
    );
    assert_eq!(
        json!(init),
        reference["suggestions"]["ReplacementIndexData"]
    );
}

fn style_type(t: &tg_codegen::style::structure_types::Type) -> (u32, Vec<String>) {
    use tg_codegen::style::structure_types::Type;
    match t {
        Type::Int => (1, vec![]),
        Type::Bool => (2, vec![]),
        Type::Double => (3, vec![]),
        Type::Pixels => (4, vec![]),
        Type::String => (5, vec![]),
        Type::Color => (6, vec![]),
        Type::Point => (7, vec![]),
        Type::Size => (8, vec![]),
        Type::Align => (9, vec![]),
        Type::Margins => (10, vec![]),
        Type::Font => (11, vec![]),
        Type::Icon => (12, vec![]),
        Type::Struct(n) => (13, vec![n.clone()]),
    }
}
fn style_value(v: &tg_codegen::style::structure_types::Value, root: &Path) -> Value {
    use tg_codegen::style::structure_types::{Data, Type};
    let (t, name) = style_type(&v.ty);
    let copy: Vec<_> = if v.copy_of.is_empty() {
        vec![]
    } else {
        v.copy_of.split('.').collect()
    };
    let data = match &v.data {
        Data::Number(n) => {
            if n.fract() == 0.0 {
                json!(*n as i64)
            } else {
                json!(n)
            }
        }
        Data::Bool(b) => json!(b),
        Data::Text(s) => json!(s),
        Data::Tuple(p) => json!(p),
        Data::Color { rgba, fallback } => json!([rgba[0], rgba[1], rgba[2], rgba[3], fallback]),
        Data::Font {
            family,
            size,
            flags,
        } => json!([family, size, flags]),
        Data::Struct(fields) => json!(fields
            .iter()
            .map(|f| json!([[f.name], style_value(&f.value, root), f.status as u32]))
            .collect::<Vec<_>>()),
        Data::Icon(parts) => json!(parts
            .iter()
            .map(|p| {
                let name = if p.filename.starts_with("size://") {
                    p.filename.clone()
                } else {
                    Path::new(&p.filename)
                        .strip_prefix(root)
                        .unwrap()
                        .display()
                        .to_string()
                };
                json!([
                    name,
                    style_value(&p.color, root),
                    style_value(
                        &tg_codegen::style::structure_types::Value::new(
                            Type::Margins,
                            Data::Tuple(p.padding.to_vec())
                        ),
                        root
                    )
                ])
            })
            .collect::<Vec<_>>()),
        Data::Default => {
            return style_value(
                &tg_codegen::style::structure_types::Value::uninitialized(v.ty.clone()),
                root,
            )
        }
    };
    json!([t, name, copy, data])
}
#[test]
fn original_all_style_structs_fields_values_copies_and_inheritance() {
    let original: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/reference/style-ir.json")).unwrap();
    assert_eq!(original.len(), 76);
    let root = root();
    let mut parser = tg_codegen::style::Parser::new(vec![
        root.join("SourceFiles"),
        root.join("lib_ui"),
        root.join("Resources"),
    ]);
    for expected in original {
        let path = expected[0].as_str().unwrap();
        let m = parser.parse(&root.join(path)).unwrap();
        let structs: Vec<_> = m
            .structs
            .iter()
            .map(|s| {
                json!([
                    [s.name],
                    s.fields
                        .iter()
                        .map(|(n, t)| {
                            let (t, name) = style_type(t);
                            json!([[n], t, name])
                        })
                        .collect::<Vec<_>>()
                ])
            })
            .collect();
        assert_eq!(json!(structs), expected[1], "structs in {path}");
        assert_eq!(
            m.variables.len(),
            expected[2].as_array().unwrap().len(),
            "variables in {path}"
        );
        for (v, expected) in m.variables.iter().zip(expected[2].as_array().unwrap()) {
            let actual = json!([[v.name], style_value(&v.value, &root), v.description]);
            assert!(
                actual == *expected,
                "{path}: {}\nactual: {}\nexpected: {}",
                v.name,
                actual.to_string().chars().take(700).collect::<String>(),
                expected.to_string().chars().take(700).collect::<String>()
            );
        }
    }
}

#[test]
fn original_historical_alias_map_postfix_required_and_emoticons() {
    let input = tg_codegen::emoji::data_read::read_data(
        &std::fs::read(root().join("lib_ui/emoji.txt")).unwrap(),
    )
    .unwrap();
    let data = tg_codegen::emoji::data::prepare(&input, &[]).unwrap();
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/reference/emoji-ir.json")).unwrap();
    let map: Vec<_> = data
        .map
        .iter()
        .rev()
        .map(|(key, index)| json!([key, index]))
        .collect();
    let required: Vec<_> = data.postfix_required.iter().copied().collect();
    let replaces: Vec<_> = data
        .replaces
        .iter()
        .rev()
        .map(|(key, index)| json!([key, index]))
        .collect();
    assert_eq!(json!([map, required, replaces]), expected);
}
