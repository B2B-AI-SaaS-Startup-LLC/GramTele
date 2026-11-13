use std::{fs, path::PathBuf};
use tg_cache::{
    storage_encrypted_file::{File, Mode, Result},
    storage_encryption::EncryptionKey,
};

fn pattern(size: usize, start: u8) -> Vec<u8> {
    (0..size).map(|v| start.wrapping_add(v as u8)).collect()
}
fn main() {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("provide a new synthetic output directory"),
    );
    fs::create_dir(&path).expect("output directory must not exist");
    for size in [64, 256] {
        let key = EncryptionKey::new((0..size).map(|v| v as u8).collect());
        let filename = path.join(format!("key{size}.tdef"));
        let mut file = File::default();
        assert_eq!(file.open(&filename, Mode::Write, &key), Result::Success);
        assert!(file.write(&mut pattern(32, 0)));
        assert!(file.write_with_padding(&mut pattern(19, 0x80)));
        assert!(file.seek(16));
        assert!(file.write(&mut pattern(16, 0x40)));
        assert!(file.flush());
        file.close();
        assert_eq!(
            file.open(&filename, Mode::ReadAppend, &key),
            Result::Success
        );
        assert!(file.seek(file.size()));
        assert!(file.write(&mut pattern(16, 0xE0)));
        assert!(file.flush());
        println!("synthetic key{size}: {} bytes", file.size());
    }
}
