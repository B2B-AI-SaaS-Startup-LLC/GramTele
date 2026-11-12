//! Port of `lib_base/base/base_file_utilities.h` and
//! `base_file_utilities.cpp`.
//!
//! Not ported: `RegisterBundledResources` — it registers a Qt resource
//! file (`QResource::registerResource`); Qt resources are replaced by
//! generated Rust data (PLAN.md, rule 3), and on Linux the original only
//! raises `Unexpected("BundledResourcesPath not implemented.")`.

/// Characters replaced by '_' in addition to control characters.
///
/// LTR/RTL marks, embeddings, overrides and isolates are a security issue in
/// file names: an executable "Fil[x]gepj.exe" may look like "Filexe.jpeg".
const BAD: [char; 17] = [
    '\u{200E}', // LTR Mark
    '\u{200F}', // RTL Mark
    '\u{202A}', // LTR Embedding
    '\u{202B}', // RTL Embedding
    '\u{202D}', // LTR Override
    '\u{202E}', // RTL Override
    '\u{2066}', // LTR Isolate
    '\u{2067}', // RTL Isolate
    '/', '\\', '<', '>', ':', '"', '|', '?', '*',
];

/// `base::FileNameFromUserString`.
///
/// The original works on UTF-16 code units; every replaced character is in
/// the BMP, so working on `char`s gives the same result. Characters outside
/// the BMP are never replaced (their surrogate halves are not in the list
/// and are >= 32).
#[must_use]
pub fn file_name_from_user_string(name: &str) -> String {
    let mut result: String = name
        .chars()
        .map(|ch| {
            if (ch as u32) < 32 || BAD.contains(&ch) {
                '_'
            } else {
                ch
            }
        })
        .collect();
    if result.is_empty() || result.ends_with(' ') || result.ends_with('.') {
        result.push('_');
    }
    platform_file_name_from_user_string(result)
}

/// `Platform::FileNameFromUserString` on Linux and macOS returns the name
/// unchanged (`base_file_utilities_linux.cpp`, `base_file_utilities_mac.mm`).
#[cfg(not(windows))]
fn platform_file_name_from_user_string(name: String) -> String {
    name
}

/// `Platform::FileNameFromUserString` on Windows (masking `.lnk`/`.scf`,
/// reserved device names), copied from `base/platform/win`.
#[cfg(windows)]
fn platform_file_name_from_user_string(name: String) -> String {
    windows_file_name(name)
}

#[cfg(any(windows, test))]
fn windows_file_name(mut name: String) -> String {
    for extension in [".lnk", ".scf"] {
        if name
            .get(name.len().saturating_sub(extension.len())..)
            .is_some_and(|tail| tail.eq_ignore_ascii_case(extension))
        {
            name.push_str(".download");
        }
    }
    for bad in [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ] {
        if name
            .get(..bad.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(bad))
            && (name.len() == bad.len() || name.as_bytes()[bad.len()] == b'.')
        {
            name.insert(0, '_');
            break;
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::file_name_from_user_string;

    #[test]
    fn windows_names() {
        for (input, expected) in [
            ("CON", "_CON"),
            ("con.txt", "_con.txt"),
            ("com1.exe", "_com1.exe"),
            ("conman", "conman"),
            ("link.LNK", "link.LNK.download"),
            ("file.scf", "file.scf.download"),
            ("😀.txt", "😀.txt"),
        ] {
            assert_eq!(super::windows_file_name(input.into()), expected);
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn sanitizes() {
        assert_eq!(
            file_name_from_user_string("a/b\\c:d*e?.txt"),
            "a_b_c_d_e_.txt"
        );
        assert_eq!(
            file_name_from_user_string("Fil\u{202E}gepj.exe"),
            "Fil_gepj.exe"
        );
        assert_eq!(file_name_from_user_string("tab\there\n"), "tab_here_");
        assert_eq!(file_name_from_user_string(""), "_");
        assert_eq!(file_name_from_user_string("name."), "name._");
        assert_eq!(file_name_from_user_string("name "), "name _");
        assert_eq!(file_name_from_user_string("файл 😀.jpg"), "файл 😀.jpg");
        assert_eq!(file_name_from_user_string("\u{2067}x\u{2066}"), "_x_");
        // U+202C (pop directional formatting) is not in the list.
        assert_eq!(file_name_from_user_string("a\u{202C}b"), "a\u{202C}b");
    }
}
