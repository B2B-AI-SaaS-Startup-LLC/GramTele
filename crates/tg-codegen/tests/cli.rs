use std::path::{Path, PathBuf};
fn files(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(root).unwrap() {
        let p = e.unwrap().path();
        if p.is_file() {
            out.push((
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(p).unwrap(),
            ));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}
fn run(bin: &str, args: &[String]) {
    let output = std::process::Command::new(bin).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn original_cli_parameters_two_runs_have_identical_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/Telegram");
    let out = std::env::temp_dir().join(format!("tg-codegen-cli-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let str_path = |p: PathBuf| p.display().to_string();
    let cases = [
        (
            env!("CARGO_BIN_EXE_codegen_style"),
            vec![
                format!("-I{}", root.join("lib_ui").display()),
                format!("-I{}", root.join("Resources").display()),
                format!("-o{}", out.join("style").display()),
                format!("-t{}", out.join("style/stamp").display()),
                str_path(root.join("lib_ui/ui/basic.style")),
            ],
            "style",
        ),
        (
            env!("CARGO_BIN_EXE_codegen_lang"),
            vec![
                "-o".into(),
                str_path(out.join("lang")),
                str_path(root.join("Resources/langs/lang.strings")),
            ],
            "lang",
        ),
        (
            env!("CARGO_BIN_EXE_codegen_emoji"),
            vec![
                format!("-o{}", out.join("emoji").display()),
                str_path(root.join("lib_ui/emoji.txt")),
                str_path(root.join("lib_ui/emoji_suggestions/emoji_autocomplete.json")),
            ],
            "emoji",
        ),
    ];
    for (bin, args, name) in cases {
        run(bin, &args);
        let first = files(&out.join(name));
        assert!(first.iter().any(|(n, _)| n.ends_with(".rs")));
        run(bin, &args);
        assert_eq!(
            first,
            files(&out.join(name)),
            "{name} output changed on repeat"
        );
    }
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_codegen_lang"))
        .args(["--subsets-only", "input.strings"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unnecessary for Rust"));
    std::fs::remove_dir_all(out).unwrap();
}
