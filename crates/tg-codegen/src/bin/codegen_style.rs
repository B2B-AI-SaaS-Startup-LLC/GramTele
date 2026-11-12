use tg_codegen::{
    common::{cpp_file, Error, Result},
    style,
};
fn run() -> Result<()> {
    let o = style::options::parse(&std::env::args().skip(1).collect::<Vec<_>>())?;
    if o.render_svg.is_some() {
        return Err(Error::new(
            "Command Line",
            0,
            910,
            "--render-svg deferred to stage 5: SVG backend is not selected in PLAN.md",
        ));
    }
    for (name, text) in style::processor::process(&o.inputs, o.includes)? {
        cpp_file::write(&o.output.join(name), &text)?;
    }
    cpp_file::touch_timestamp(&o.timestamp)
}
fn main() {
    if let Err(error) = run() {
        if error.code != 0 {
            eprintln!("{error}");
        }
        std::process::exit(if error.code == 910 { 1 } else { -1 });
    }
}
