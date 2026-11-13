//! Generate synthetic databases or read copies of native databases.
#[path = "../tests/support/database.rs"]
mod support;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "generate|verify directory");
    let generate = args[1] == "generate";
    let root = std::path::Path::new(&args[2]);
    if generate {
        assert!(!root.exists(), "new synthetic directory required");
        std::fs::create_dir_all(root).unwrap();
    }
    for name in support::SCENARIOS {
        support::scenario(root, name, generate);
        println!("{name} {} PASS", args[1]);
    }
}
