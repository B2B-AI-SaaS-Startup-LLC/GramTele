use tg_codegen::{
    common::{cpp_file, Error, Result},
    emoji,
};
fn read(path: &str) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|_| Error::new(path, 0, 101, ": error: file does not exist."))
}
fn run() -> Result<()> {
    let o = emoji::options::parse(&std::env::args().skip(1).collect::<Vec<_>>())?;
    if o.images.as_ref().is_some_and(|s| !s.is_empty()) {
        return Err(Error::new(
            "Command Line",
            0,
            910,
            "--images deferred to stage 5: use the existing emoji_*.webp resources",
        ));
    }
    let mut old = Vec::new();
    for path in &o.old {
        old.push(emoji::data_read::read_data(&read(path)?)?);
    }
    let text = emoji::processor::process(&read(&o.data)?, &read(&o.replaces)?, &o.replaces, &old)?;
    let path = std::path::Path::new(&o.output);
    cpp_file::write(&path.join("emoji.rs"), &text)?;
    cpp_file::touch_timestamp(&path.join("emoji"))
}
fn main() {
    if let Err(error) = run() {
        if error.code != 0 {
            eprintln!("{error}");
        }
        std::process::exit(if error.code == 910 { 1 } else { -1 });
    }
}
