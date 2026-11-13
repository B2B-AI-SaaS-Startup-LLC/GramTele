//! Independent ZIP extraction. Main::Session loader binding and UI text are deferred.
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
use tg_base::zlib_help::{FileToRead, UNZ_END_OF_LIST_OF_FILE, UNZ_OK};
pub const CLOUD_LOCATION_USERNAME: &str = "tdhbcfiles";
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Blob {
    pub id: i32,
    pub post_id: i32,
    pub size: i64,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlobState {
    Available { size: i64 },
    Ready,
    Active,
    Failed,
    Loading { already: i64, size: i64 },
}
fn clean_path(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|p| *p != "..") {
                    parts.pop();
                } else if !absolute {
                    parts.push("..");
                }
            }
            _ => parts.push(part),
        }
    }
    let joined = parts.join("/");
    if absolute {
        format!("/{joined}")
    } else if joined.is_empty() {
        ".".into()
    } else {
        joined
    }
}
fn read_final_file(path: &Path) -> Option<Vec<u8>> {
    let mut file = fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > 10 * 1024 * 1024 {
        return None;
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}
fn extract_zip_file(zip: &mut FileToRead, path: &Path) -> bool {
    let content = zip.read_current_file_content(25 * 1024 * 1024);
    !content.is_empty()
        && zip.error() == UNZ_OK
        && fs::File::create(path)
            .and_then(|mut f| f.write_all(&content))
            .is_ok()
}
pub fn unpack_blob(path: &Path, folder: &str, mut check_name: impl FnMut(&str) -> bool) -> bool {
    let Some(bytes) = read_final_file(path).filter(|v| !v.is_empty()) else {
        return false;
    };
    let mut zip = FileToRead::new(bytes);
    if zip.go_to_first_file() != UNZ_OK {
        return false;
    }
    let clean_folder = clean_path(folder);
    loop {
        let name = zip.get_current_file_name();
        let path = format!("{folder}/{name}");
        let clean = clean_path(&path);
        let inside = clean == clean_folder || clean.starts_with(&format!("{clean_folder}/"));
        if check_name(&name) && (!inside || !extract_zip_file(&mut zip, Path::new(&path))) {
            return false;
        }
        match zip.go_to_next_file() {
            UNZ_END_OF_LIST_OF_FILE => return true,
            UNZ_OK => {}
            _ => return false,
        }
    }
}
