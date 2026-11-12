//! Non-window functions from base_file_utilities_linux.cpp.
//! ShowInFolder (portal/file manager) and BundledResourcesPath remain deferred.
use std::path::{Path, PathBuf};

pub fn current_executable_path(arguments: &[String]) -> PathBuf {
    if let Ok(path) = std::fs::canonicalize(format!("/proc/{}/exe", std::process::id())) {
        return path;
    }
    let Some(first) = arguments.first() else {
        return PathBuf::new();
    };
    if !first.is_empty() && !first.contains('/') {
        use std::os::unix::fs::PermissionsExt;
        for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
            let path = directory.join(first);
            if let Ok(metadata) = path.metadata() {
                if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
                    return path;
                }
            }
        }
    }
    let path = PathBuf::from(first);
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    }
}
/// The original Linux function deliberately does nothing.
pub fn remove_quarantine(_path: &Path) {}
pub fn file_name_from_user_string(name: String) -> String {
    name
}
pub fn rename_with_overwrite(from: &Path, to: &Path) -> bool {
    std::fs::rename(from, to).is_ok()
}
pub fn flush_file_data(file: &mut std::fs::File) {
    use std::io::Write;
    use std::os::fd::AsRawFd;
    let _ = file.flush();
    // Preserve the original `if (descriptor)` check, including descriptor zero.
    if file.as_raw_fd() != 0 {
        let _ = file.sync_all();
    }
}
/// Like stat()+recursive DeleteDirectory in C++, this follows directory symlinks.
/// It can delete their contents and then fail to remove the symlink as a directory.
pub fn delete_directory(path: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(path) else {
        return false;
    };
    for entry in entries {
        let Ok(entry) = entry else { return false };
        let path = entry.path();
        if let Ok(metadata) = std::fs::metadata(&path) {
            if metadata.is_dir() {
                if !delete_directory(&path) {
                    return false;
                }
            } else if std::fs::remove_file(&path).is_err() {
                return false;
            }
        }
    }
    std::fs::remove_dir(path).is_ok()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filesystem_behaviour() {
        let root = std::env::temp_dir().join(format!("tg-platform-fs-{}", std::process::id()));
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("old"), b"old").unwrap();
        std::fs::write(root.join("new"), b"new").unwrap();
        assert!(rename_with_overwrite(&root.join("old"), &root.join("new")));
        assert_eq!(std::fs::read(root.join("new")).unwrap(), b"old");
        flush_file_data(
            &mut std::fs::OpenOptions::new()
                .write(true)
                .open(root.join("new"))
                .unwrap(),
        );
        assert_eq!(file_name_from_user_string("a:b?".into()), "a:b?");
        remove_quarantine(&root);
        assert!(delete_directory(&root));
        assert!(!delete_directory(&root));
        assert_eq!(
            current_executable_path(&[]),
            std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap()
        );
    }
    #[test]
    fn directory_symlink_retains_original_failure() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("tg-platform-link-{}", std::process::id()));
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::create_dir_all(root.join("source")).unwrap();
        std::fs::write(root.join("target/data"), b"data").unwrap();
        symlink(root.join("target"), root.join("source/link")).unwrap();
        assert!(!delete_directory(&root.join("source")));
        assert!(!root.join("target/data").exists());
        assert!(root.join("source/link").is_symlink());
        std::fs::remove_dir_all(root).unwrap();
    }
}
