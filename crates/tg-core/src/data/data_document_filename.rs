//! Pure naming portion of data_document.cpp. Dialogs, settings mutations,
//! default download directory and filesystem access belong to their services.
use super::data_document::DocumentData;
use crate::core::mime_type::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveCaption {
    Audio,
    AudioFile,
    Video,
    File,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentSaveName {
    pub name: String,
    pub filter: String,
    pub prefix: &'static str,
    pub caption: SaveCaption,
}
impl DocumentData {
    /// DocumentFileNameForSave before UI/filesystem effects. Caption is a real
    /// translation request; lang 01 supplies the text when presenting a dialog.
    pub fn file_name_for_save(
        &self,
        name: &str,
        database: &impl MimeDatabase,
        all_files_filter: &str,
    ) -> DocumentSaveName {
        let mime = mime_type_for_name(self.mime_string(), database);
        let already = name;
        let mut name = if name.is_empty() {
            self.filename().to_owned()
        } else {
            name.to_owned()
        };
        if self.is_voice_message() {
            let mp3 = self.has_mime_type("audio/mp3");
            name = if already.is_empty() {
                if mp3 { ".mp3" } else { ".ogg" }.into()
            } else {
                already.into()
            };
            return DocumentSaveName {
                name,
                filter: format!(
                    "{};;{all_files_filter}",
                    if mp3 {
                        "MP3 Audio (*.mp3)"
                    } else {
                        "OGG Opus Audio (*.ogg)"
                    }
                ),
                prefix: "audio",
                caption: SaveCaption::Audio,
            };
        }
        let pattern = mime.glob_patterns.first().filter(|p| !p.is_empty());
        if name.is_empty() {
            name = pattern.map_or_else(
                || {
                    if self.is_video_file() {
                        ".mov"
                    } else {
                        ".unknown"
                    }
                    .into()
                },
                |p| p.replace('*', ""),
            );
        }
        let filter = if pattern.is_some() {
            format!("{};;{all_files_filter}", mime.filter_string)
        } else if self.is_video_file() {
            format!("MOV Video (*.mov);;{all_files_filter}")
        } else {
            String::new()
        };
        DocumentSaveName {
            name,
            filter,
            prefix: if self.is_video_file() { "video" } else { "doc" },
            caption: if self.is_video_file() {
                SaveCaption::Video
            } else if self.is_audio_file() {
                SaveCaption::AudioFile
            } else {
                SaveCaption::File
            },
        }
    }
}
pub fn file_name_for_save_platform(result: &str, platform: FilenamePlatform) -> String {
    // FileNameForSave deliberately uses toLower + case-sensitive endsWith,
    // unlike Platform::FileNameFromUserString's CI check. In particular ſ
    // remains ſ here (native savedmask cases), while Kelvin lowercases to k.
    let lower = result.trim().to_lowercase();
    if platform == FilenamePlatform::Windows && (lower.ends_with(".lnk") || lower.ends_with(".scf"))
    {
        format!("{result}.download")
    } else {
        result.into()
    }
}
/// First filter extension promotion, exactly the pure branch in FileNameUnsafe.
pub fn promote_save_filter(name: &str, filter: &str) -> String {
    let ext = file_extension(name);
    if ext.is_empty() {
        return filter.into();
    }
    if !ext.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
        return String::new();
    }
    let parts: Vec<_> = filter.split(";;").collect();
    if parts.len() <= 1 {
        return String::new();
    }
    let first = parts[0];
    let Some(start) = first.find("(*.") else {
        return String::new();
    };
    let matches = |prefix: &str| {
        first.match_indices(prefix).find_map(|(i, _)| {
            let rest = &first[i + prefix.len()..];
            let end = qt_ascii_ci_prefix(rest, ext)?;
            rest[end..]
                .chars()
                .next()
                .is_some_and(|c| matches!(c, ')' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' '))
                .then_some((i, i + prefix.len() + end))
        })
    };
    if matches("(*.").is_some() {
        return filter.into();
    }
    let tail = if let Some((pos, end)) = matches(" *.").filter(|(pos, _)| *pos > start + 3) {
        format!("{}{}", &first[start + 3..pos], &first[end..])
    } else {
        first[start + 3..].into()
    };
    format!(
        "{}{ext} *.{tail};;{}",
        &first[..start + 3],
        parts[1..].join(";;")
    )
}
/// FileNameUnsafe's collision loop. Caller supplies the real filesystem query;
/// no filesystem or time is implicit in tests. `name` must be sanitized and
/// nonempty, and leading-dot default names are handled by the UI service.
pub fn available_file_name(path: &str, name: &str, mut exists: impl FnMut(&str) -> bool) -> String {
    assert!(!name.is_empty() && !name.starts_with('.'));
    let (base, ext) = name
        .rfind('.')
        .map_or((name, ""), |i| (&name[..i], &name[i..]));
    let mut candidate = format!("{path}{name}");
    let mut index = 1u64;
    while exists(&candidate) {
        index = index
            .checked_add(1)
            .expect("filename collision count exhausted");
        candidate = format!("{path}{base} ({index}){ext}");
    }
    candidate
}
