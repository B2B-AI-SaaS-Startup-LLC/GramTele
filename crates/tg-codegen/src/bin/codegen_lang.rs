use std::path::Path;
use tg_codegen::{
    common::{cpp_file, Error, Result},
    lang,
};
fn run() -> Result<()> {
    let o = lang::options::parse(&std::env::args().skip(1).collect::<Vec<_>>())?;
    if o.subsets_only {
        eprintln!("codegen_lang: --subsets-only is unnecessary for Rust; no C++ source scan or subset headers are produced");
        return Ok(());
    }
    let bytes = std::fs::read(&o.input)
        .map_err(|_| Error::new(&o.input, 0, 101, ": error: file does not exist."))?;
    let path = Path::new(&o.output);
    let tags = std::fs::read_to_string(path.join("lang_auto.tags")).unwrap_or_default();
    let indices = std::fs::read_to_string(path.join("lang_auto.indices")).unwrap_or_default();
    let out = lang::processor::process(&bytes, &o.input, &tags, &indices)?;
    for (name, text) in [
        ("lang_auto.rs", out.rust),
        ("lang_auto.tags", out.tags),
        ("lang_auto.indices", out.indices),
    ] {
        cpp_file::write(&path.join(name), &text)?;
    }
    cpp_file::touch_timestamp(&path.join("lang_auto"))
}
fn main() {
    if let Err(error) = run() {
        if error.code != 0 {
            eprintln!("{error}");
        }
        std::process::exit(if error.code == 910 { 1 } else { -1 });
    }
}
