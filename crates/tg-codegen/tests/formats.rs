use std::path::PathBuf;
use tg_codegen::{
    common::{Kind, TokenizedFile},
    lang,
};
#[test]
fn tokenizer_strings_comments_numbers() {
    let f = TokenizedFile::parse(
        b"// first\n123 12px .5 1. -4 /* x\n y */ \"a\\nb\\q\\t\\\"\\\\\";",
        "sample",
    )
    .unwrap();
    assert_eq!(f.tokens[0].line, 2);
    assert_eq!(f.tokens[0].kind, Kind::Int);
    assert_eq!(f.tokens[1].kind, Kind::Name);
    assert_eq!(f.tokens[2].kind, Kind::Double);
    assert_eq!(f.tokens[2].value, ".5");
    assert_eq!(f.tokens[3].value, "1.");
    assert_eq!(f.tokens[6].value, "a\nb\t\"\\");
    assert_eq!(f.tokens[6].line, 3);
}
#[test]
fn tokenizer_errors() {
    for (bytes, expected) in [
        (
            b"\"a\n".as_slice(),
            "sample(1): error 201: unterminated string literal.",
        ),
        (
            b"\"\xef\xbf\xbd\"",
            "sample(1): error 202: incorrect UTF-8 string literal.",
        ),
        (b"/", "sample(1): error 203: incorrect token '/'"),
        (b"/*", "sample: error 104: "),
    ] {
        assert_eq!(
            TokenizedFile::parse(bytes, "sample")
                .err()
                .unwrap()
                .to_string(),
            expected
        );
    }
}
#[test]
fn comments_inside_strings_and_crlf() {
    let f =
        TokenizedFile::parse(b"\"// text /* */\"; // description\r\n\"next\";", "sample").unwrap();
    assert_eq!(f.tokens[0].value, "// text /* */");
    assert_eq!(f.tokens[2].line, 2);
    assert_eq!(f.comments[0], b"// description");
}
#[test]
fn lang_tags_plural_and_utf16() {
    let input="\"hello\" = \"Hi {user} 😀\";\n\"n#one\" = \"{count} item\";\n\"n#other\" = \"{count} items by {user}\";\n\"last\" = \"end\";";
    let p = lang::parse(input.as_bytes(), "sample", &[]).unwrap();
    assert_eq!(p.entries.len(), 8);
    assert_eq!(p.tags, ["user", "count"]);
    assert_eq!(
        p.entries[0].value,
        vec![72, 105, 32, 16, 32, 32, 16, 32, 0xd83d, 0xde00]
    );
    assert_eq!(p.entries[1].key, "n__plural0");
    assert!(p.entries[1].value.is_empty());
    assert_eq!(
        p.entries[2].value,
        vec![16, 32, 33, 16, 32, 105, 116, 101, 109]
    );
    assert_eq!(p.entries[6].tags, ["count", "user"]);
}
#[test]
fn lang_expected_errors() {
    for (input, expected) in [
        (
            "\"a\" = \"{bad-tag}\";",
            "sample(1): error 806: bad tag characters: 'bad-tag'",
        ),
        (
            "\"a\" = \"{x} {x}\";",
            "sample(1): error 806: duplicate found for tag 'x'",
        ),
        (
            "\"a\" = \"{x\";",
            "sample(1): error 806: unexpected end of value, end of tag expected.",
        ),
        (
            "\"a\" = \"{x:#|#}\";",
            "sample(1): error 806: bad option count for plural key part in tag: 'x:#|#'",
        ),
        (
            "\"a\" = \"{count}\";",
            "sample(1): error 806: plural tag 'count' used in non-plural key 'a'",
        ),
        (
            "\"a\" = \"a\"; \"a\" = \"b\";",
            "sample(1): error 806: duplicate found for key 'a'",
        ),
    ] {
        assert_eq!(
            lang::parse(input.as_bytes(), "sample", &[])
                .unwrap_err()
                .to_string(),
            expected
        );
    }
}
#[test]
fn original_plural_at_end_assertion() {
    let error = lang::parse(b"\"n#other\" = \"{count}\";", "sample", &[]).unwrap_err();
    assert_eq!(error.code, 666);
}
#[test]
fn legacy_numeric_tag_expansion() {
    let p = lang::parse(b"\"a\" = \"{n:none|# one|# many}\";", "sample", &[]).unwrap();
    assert_eq!(
        p.entries.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(),
        ["a", "a__n0", "a__n1", "a__n2"]
    );
    assert_eq!(
        p.entries[1].value,
        b"none".iter().map(|&b| b as u16).collect::<Vec<_>>()
    );
    assert_eq!(&p.entries[2].value[..4], &[16, 32, 32, 16]);
}
#[test]
fn stable_lang_slots_and_holes() {
    let p = lang::parse(b"\"new\"=\"N\";\"old\"=\"O\";", "sample", &[]).unwrap();
    let g = lang::generate(&p, "old\t2\t1\nremoved\t8\t1\n", "sample").unwrap();
    assert_eq!(g.keys_count, 10);
    assert_eq!(g.indices, "new\t9\t1\nold\t2\t1\nremoved\t8\t1\n");
    assert_eq!(
        g.rust,
        lang::generate(&p, &g.indices, "sample").unwrap().rust
    );
}
fn style_file(name: &str, text: &str) -> (PathBuf, tg_codegen::style::Parser) {
    let root =
        std::env::temp_dir().join(format!("tg-codegen-format-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join(name);
    std::fs::write(&file, text).unwrap();
    (file, tg_codegen::style::Parser::new(vec![root]))
}
#[test]
fn palette_exact_checksum_input_and_indices() {
    let (file, mut parser) = style_file("test.palette", "a:#010203; b:a; c:#04050607|a;");
    let m = parser.parse(&file).unwrap();
    let p = tg_codegen::style::generator::palette(&m).unwrap();
    assert_eq!(p.checksum_input,"&transparent:{ 255, 255, 255, 0 }&white:{ 255, 255, 255, 255 }&a:{ 1, 2, 3, 255 }&b:{ 1, 2, 3, 255 }&c:{ 4, 5, 6, 7 }");
    assert_eq!(
        p.colors.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["transparent", "white", "a", "b", "c"]
    );
    assert_eq!(p.checksum as u32, 0x2dedc739);
    assert_eq!(p.colors[3].fallback, 2);
    assert_eq!(p.colors[4].fallback, 2);
    assert_eq!(p.colors[4].index, 4);
    std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
}
#[test]
fn style_structure_inheritance_arithmetic() {
    let(file,mut parser)=style_file("struct.style","S { x:pixels; y:point; }\na: 3px + 2px;\np: S { x:a; y:point(2px,4px); }\nq:S(p) { x:-1px; }\nz:q.y.x;");
    let m = parser.parse(&file).unwrap();
    assert_eq!(m.variables[0].value.number(), 5.0);
    assert_eq!(m.variables.last().unwrap().value.number(), 2.0);
    let code = tg_codegen::style::generator::generate(&m).unwrap();
    assert!(code.contains("r#q()"));
    std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
}
#[test]
fn style_errors_and_cycle() {
    let (file, mut parser) = style_file("bad.style", "p:point(1,2);");
    let e = parser.parse(&file).unwrap_err();
    assert_eq!(e.code, 802);
    assert_eq!(
        e.message,
        "type mismatch: expected two px values for the point"
    );
    assert_eq!(e.line, 1);
    std::fs::write(&file, "using \"bad.style\";").unwrap();
    assert_eq!(parser.parse(&file).unwrap_err().code, 809);
    std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
}
#[test]
fn style_duplicate_and_unknown_field() {
    for (name, input, code) in [
        ("duplicate.style", "x:1;x:2;", 805),
        ("field.style", "S { x:int; } a:S { y:1; }", 803),
        ("unicode.style", "x:\"Ж\";", 806),
    ] {
        let (file, mut parser) = style_file(name, input);
        assert_eq!(parser.parse(&file).unwrap_err().code, code);
        std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
    }
}
#[test]
fn packed_utf16_ids() {
    let id = tg_codegen::emoji::data_read::input_id_from_string("😀\u{fe0f}\u{200d}❤");
    assert_eq!(id, [0xd83dde00, 0xfe0f, 0x200d, 0x2764]);
    assert_eq!(
        tg_codegen::emoji::data::bare(&id),
        [0xd83d, 0xde00, 0x200d, 0x2764]
    );
}
#[test]
fn emoji_errors() {
    assert_eq!(
        tg_codegen::emoji::data_read::read_data(b"\"x\"")
            .unwrap_err()
            .to_string(),
        "input: error 401: Bad data: Wrong file parts."
    );
    let json = br#"{"a":{"output":"d800","name":"x","alpha_code":":x:","aliases":""}}"#;
    assert_eq!(
        tg_codegen::emoji::replaces::prepare(json, "sample")
            .unwrap_err()
            .to_string(),
        "sample: error 402: Bad data: Child 'a' output part invalid: d800"
    );
    assert_eq!(
        tg_codegen::emoji::replaces::prepare(b"[]", "sample")
            .unwrap_err()
            .message,
        "Bad data: Root object not found."
    );
}
#[test]
fn replacement_words_and_checksums() {
    assert_eq!(
        tg_codegen::emoji::replaces::words(":one-2_one+3_one:"),
        ["one", "one+3", "one-2"]
    );
    assert_eq!(
        tg_codegen::emoji::replaces::count_checksum("123456789"),
        0xa290e877
    );
    assert_eq!(
        tg_codegen::emoji::replaces::count_checksum("😀"),
        0xc1f4643b
    );
}
#[test]
fn cli_error_messages() {
    assert_eq!(
        lang::options::parse(&["-o".into()])
            .unwrap_err()
            .to_string(),
        "Command Line: error 902: output path expected after -o"
    );
    assert_eq!(
        tg_codegen::style::options::parse(&[])
            .unwrap_err()
            .to_string(),
        "Command Line: error 903: timestamp path expected"
    );
    assert_eq!(
        tg_codegen::emoji::options::parse(&["-oout".into(), "emoji".into()])
            .unwrap_err()
            .message,
        "replaces path expected"
    );
}

#[test]
fn native_qt_json_diagnostics_and_unknown_escapes() {
    // These messages were captured from the native codegen_emoji, Qt 6.11.2.
    for (input, message) in [
        ("{", "Could not parse data (1): unterminated object"),
        ("[", "Could not parse data (3): unterminated array"),
        ("{\"a\":", "Could not parse data (1): unterminated object"),
        (
            "{\"a\" 1}",
            "Could not parse data (2): missing name separator",
        ),
        (
            "{\"a\":1 \"b\":2}",
            "Could not parse data (1): unterminated object",
        ),
        (
            "{\"a\":\"x}",
            "Could not parse data (10): unterminated string",
        ),
        (
            "{\"a\":1,}",
            "Could not parse data (11): object is missing after a comma",
        ),
        ("{\"a\":x}", "Could not parse data (7): illegal number"),
        (
            "{} x",
            "Could not parse data (14): garbage at the end of the document",
        ),
        ("null", "Could not parse data (5): illegal value"),
    ] {
        assert_eq!(
            tg_codegen::common::qt_json::parse(input.as_bytes(), "sample")
                .unwrap_err()
                .message,
            format!("Bad data: {message}")
        );
    }
    assert_eq!(
        tg_codegen::common::qt_json::parse(br#"{"a":"\q"}"#, "sample")
            .unwrap()
            .as_object()
            .unwrap()
            .get(&vec![97])
            .unwrap()
            .string()
            .unwrap(),
        "q"
    );
}

#[test]
fn ayugram_wide_policy_and_scale_are_emitted() {
    let (file, mut parser) = style_file(
        "ayu.style",
        "msgMaxWidth:440px; maxGifSize:size(100px,50px); offset:point(2px,3px);",
    );
    let m = parser.parse(&file).unwrap();
    let text = tg_codegen::style::generator::generate(&m).unwrap();
    assert!(text.contains("wide_multiplied(tg_ui::style::convert_scale(440i32), 1.0)"));
    assert!(text.contains("wide_multiplied(tg_ui::style::convert_scale(100), 0.6)"));
    assert!(
        text.contains("Point::new(tg_ui::style::convert_scale(2),tg_ui::style::convert_scale(3))")
    );
    std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
}
#[test]
fn native_replacement_empty_alias_is_rejected() {
    let json = br#"{"a":{"output":"1f600","name":"x","alpha_code":":x:","aliases":":a:||:b:"}}"#;
    assert_eq!(
        tg_codegen::emoji::replaces::prepare(json, "sample")
            .unwrap_err()
            .message,
        "Bad data: Child 'a' alias invalid: "
    );
}

#[test]
fn clean_file_preserves_comment_whitespace_and_crlf() {
    let c = tg_codegen::common::clean_file::CleanFile::parse(b"a/*b\r\nc*/d // x\r\n", "sample")
        .unwrap();
    assert_eq!(c.bytes, b"a \r\n d  \r\n");
}

#[test]
fn earlier_semantic_error_precedes_later_lexical_error() {
    let input = b"\"a\"=\"x\";\"a\"=\"y\";\"z\"=\"unterminated";
    assert_eq!(
        tg_codegen::lang::parse(input, "sample", &[])
            .unwrap_err()
            .message,
        "duplicate found for key 'a'"
    );
    assert_eq!(
        tg_codegen::lang::parse(b"\"a\"=\"x\";@", "sample", &[])
            .unwrap_err()
            .code,
        203
    );
}

#[test]
fn qt_json_preserves_lone_surrogates_and_qstring_key_order() {
    let value =
        tg_codegen::common::qt_json::parse(br#"{"\ud800":"\udfff","\ue000":"x"}"#, "sample")
            .unwrap();
    let object = value.as_object().unwrap();
    assert_eq!(object.keys().next().unwrap(), &vec![0xd800]);
    assert_eq!(
        object.get(&vec![0xd800]),
        Some(&tg_codegen::common::qt_json::Value::String(vec![0xdfff]))
    );
}
#[test]
fn qt_json_nesting_limit_matches_qt() {
    let ok = format!("{}{}", "[".repeat(1024), "]".repeat(1024));
    assert!(tg_codegen::common::qt_json::parse(ok.as_bytes(), "sample").is_ok());
    let bad = format!("{}{}", "[".repeat(1025), "]".repeat(1025));
    assert!(tg_codegen::common::qt_json::parse(bad.as_bytes(), "sample")
        .unwrap_err()
        .message
        .contains("(12): too deeply nested document"));
}

#[test]
fn lone_surrogates_do_not_bypass_ascii_validators() {
    let json = br#"{"a":{"output":"1f600","name":"\ud800","alpha_code":":x\ud800:","aliases":""}}"#;
    assert_eq!(
        tg_codegen::emoji::replaces::prepare(json, "sample")
            .unwrap_err()
            .message,
        "Bad data: Child 'a' alpha_code invalid: :x:"
    );
    let good = br#"{"a":{"output":"1f600","name":"\ud800","alpha_code":":x:","aliases":""}}"#;
    assert!(tg_codegen::emoji::replaces::prepare(good, "sample").is_ok());
}
