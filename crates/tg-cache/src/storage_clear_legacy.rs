//! `storage_clear_legacy` and its POSIX/Win directory adapters.
use std::{collections::BTreeSet, fs, path::Path, sync::Arc};
pub type GoodFiles = BTreeSet<String>;
pub type CollectGoodFiles = Arc<dyn Fn(Box<dyn FnOnce(GoodFiles) + Send>) + Send + Sync>;
pub const CLEAR_PART_SIZE: usize = 10000;
pub fn collect_files(base: &str, limit: usize, skip: &GoodFiles) -> Vec<String> {
    assert!(base.ends_with('/'));
    assert!(limit > 0);
    // POSIX original removes the trailing slash before opendir, even for '/'.
    #[cfg(unix)]
    let folder = &base[..base.len() - 1];
    #[cfg(not(unix))]
    let folder = base;
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for entry in entries.flatten() {
        #[cfg(unix)]
        let metadata = fs::metadata(entry.path());
        #[cfg(not(unix))]
        let metadata = entry.metadata();
        let Ok(metadata) = metadata else {
            continue;
        };
        #[cfg(windows)]
        let directory = {
            use std::os::windows::fs::MetadataExt;
            metadata.file_attributes() & 0x10 != 0
        };
        #[cfg(not(windows))]
        let directory = metadata.is_dir();
        if directory {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !skip.contains(&name) {
            result.push(name);
            if result.len() == limit {
                break;
            }
        }
    }
    result
}
pub fn remove_legacy_file(path: &Path) -> bool {
    fs::remove_file(path).is_ok()
}
fn clear_legacy_files_part(base: String, filter: CollectGoodFiles, skip: GoodFiles) {
    let files = collect_files(&base, CLEAR_PART_SIZE, &skip);
    let again = filter.clone();
    filter(Box::new(move |mut skip| {
        tg_runtime::async_(move || {
            for name in &files {
                if !skip.contains(name) && !remove_legacy_file(&Path::new(&base).join(name)) {
                    skip.insert(name.clone());
                }
            }
            if files.len() == CLEAR_PART_SIZE {
                clear_legacy_files_part(base, again, skip);
            }
        });
    }));
}
pub fn clear_legacy_files(base: String, filter: CollectGoodFiles) {
    assert!(base.ends_with('/'));
    tg_runtime::async_(move || clear_legacy_files_part(base, filter, GoodFiles::new()));
}
