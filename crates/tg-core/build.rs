fn main() {
    let root = std::path::Path::new("../tg-codegen/tests/fixtures");
    let input = root.join("Telegram/Resources/langs/lang.strings");
    let tags = root.join("reference/lang_auto.tags");
    let indices = root.join("reference/lang_auto.indices");
    for p in [&input, &tags, &indices] {
        println!("cargo:rerun-if-changed={}", p.display());
    }
    let generated = tg_codegen::lang::processor::process(
        &std::fs::read(&input).unwrap(),
        "lang.strings",
        &std::fs::read_to_string(tags).unwrap(),
        &std::fs::read_to_string(indices).unwrap(),
    )
    .unwrap();
    let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("lang_auto.rs");
    std::fs::write(path, generated.rust).unwrap();
}
