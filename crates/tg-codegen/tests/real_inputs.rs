use std::path::{Path, PathBuf};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/Telegram")
}
fn styles(root: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(root).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            styles(&p, out);
        } else if p.extension().is_some_and(|e| e == "style") {
            out.push(p);
        }
    }
}
#[test]
fn real_lang() {
    let root = root();
    let path = root.join("Resources/langs/lang.strings");
    let started = std::time::Instant::now();
    let pack =
        tg_codegen::lang::parse(&std::fs::read(&path).unwrap(), "lang.strings", &[]).unwrap();
    let generated = tg_codegen::lang::generate(&pack, "", "lang_auto").unwrap();
    println!(
        "lang entries={} tags={} slots={}",
        pack.entries.len(),
        pack.tags.len(),
        generated.keys_count
    );
    println!("generate lang: {:?}", started.elapsed());
    assert_eq!(pack.entries.len(), 10997);
    assert_eq!(pack.tags.len(), 175);
    compile(
        "lang",
        &format!("pub mod generated {{ {} }}", generated.rust),
    );
    assert_eq!(
        generated.rust,
        tg_codegen::lang::generate(&pack, "", "lang_auto")
            .unwrap()
            .rust
    );
}
#[test]
fn real_styles() {
    let root = root();
    let mut paths = Vec::new();
    styles(&root, &mut paths);
    paths.sort();
    assert_eq!(paths.len(), 76);
    let started = std::time::Instant::now();
    let mut parser = tg_codegen::style::Parser::new(vec![
        root.join("SourceFiles"),
        root.join("lib_ui"),
        root.join("Resources"),
    ]);
    for path in paths {
        parser.parse(&path).unwrap();
    }
    println!("style modules={}", parser.cache.len());

    let palette = parser.cache.values().find(|m| m.palette).unwrap();
    let p = tg_codegen::style::generator::palette(palette).unwrap();
    println!("palette colors={} checksum={}", p.colors.len(), p.checksum);
    let mut code = String::from("pub mod generated {\n");
    for m in parser.cache.values() {
        let name = tg_codegen::style::generator::module_name(m);
        let a = tg_codegen::style::generator::generate(m).unwrap();
        assert_eq!(a, tg_codegen::style::generator::generate(m).unwrap());
        code += &format!("pub mod {name} {{ {a} }}\n");
    }
    code += "}";
    println!("generate styles: {:?}", started.elapsed());
    assert_eq!(p.colors.len(), 588);
    assert_eq!(p.checksum, -1755758547);
    assert_eq!(p.colors[2].name, "windowBg");
    compile("styles", &code);
}

#[test]
fn real_emoji() {
    let root = root();
    let started = std::time::Instant::now();
    let input = tg_codegen::emoji::data_read::read_data(
        &std::fs::read(root.join("lib_ui/emoji.txt")).unwrap(),
    )
    .unwrap();
    let data = tg_codegen::emoji::data::prepare(&input, &[]).unwrap();
    let replaces = tg_codegen::emoji::replaces::prepare(
        &std::fs::read(root.join("lib_ui/emoji_suggestions/emoji_autocomplete.json")).unwrap(),
        "emoji_autocomplete.json",
    )
    .unwrap();
    let replaces =
        tg_codegen::emoji::replaces::check_and_convert(replaces, &data, "emoji_autocomplete.json")
            .unwrap();
    println!(
        "emoji={} map={} replacements={}",
        data.list.len(),
        data.map.len(),
        replaces.len()
    );
    let a = tg_codegen::emoji::generator::generate(&data, &replaces).unwrap();
    println!("generate emoji: {:?}", started.elapsed());
    assert_eq!(data.list.len(), 3736);
    assert_eq!(data.map.len(), 3767);
    assert_eq!(replaces.len(), 2018);
    assert_eq!(
        a,
        tg_codegen::emoji::generator::generate(&data, &replaces).unwrap()
    );
    compile("emoji", &format!("pub mod generated {{ {a} }}"));
}

fn compile(name: &str, code: &str) {
    let dir = std::env::temp_dir().join(format!("tg-codegen-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("generated.rs");
    let text = format!(
        "#![allow(dead_code, non_upper_case_globals, unused_imports)]\n{}\n{code}",
        include_str!("support/contracts.rs")
    );
    std::fs::write(&source, text).unwrap();
    let started = std::time::Instant::now();
    let output = std::process::Command::new("rustc")
        .arg("--edition=2021")
        .arg("--crate-type=lib")
        .arg(&source)
        .arg("-o")
        .arg(dir.join("generated.rlib"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    println!("compile {name}: {:?}", started.elapsed());
    std::fs::remove_dir_all(dir).unwrap();
}
