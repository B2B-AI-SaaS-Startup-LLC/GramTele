//! The generator invocation of `cmake/td_scheme.cmake` and
//! `cmake/generate_scheme.cmake` (`codegen_scheme.py -o.../scheme api.tl
//! mtproto.tl`); the scheme options are in `codegen_scheme.rs`.

use std::path::PathBuf;

use tg_tl_gen::Input;

include!("codegen_scheme.rs");

fn main() {
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let mut inputs = Vec::new();
    // Same order as td_scheme.cmake: api.tl, then mtproto.tl.
    for name in ["api.tl", "mtproto.tl"] {
        let path = dir.join("scheme").join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("can't read {}: {e}", path.display()));
        inputs.push(Input {
            name: name.to_string(),
            text,
        });
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=codegen_scheme.rs");
    let (schema, output) = match tg_tl_gen::generate_files(&inputs, &config()) {
        Ok(result) => result,
        Err(error) => panic!("scheme generation failed: {error}"),
    };
    for warning in &schema.warnings {
        println!("cargo:warning={warning}");
    }
    std::fs::write(out.join("scheme.rs"), output.scheme).expect("write scheme.rs");
    std::fs::write(out.join("scheme_meta.rs"), output.meta).expect("write scheme_meta.rs");
}
