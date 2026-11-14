//! Headless core/mime_type slice. System MIME database lookup is a caller
//! supplied service, not a guessed list of extensions (remaining package 33).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum NameType {
    #[default]
    Unknown,
    Executable,
    Image,
    Video,
    Audio,
    Document,
    Archive,
    ThemeFile,
    OtherBenign,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilenamePlatform {
    Linux,
    Windows,
    Mac,
}
impl FilenamePlatform {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Mac
        } else {
            Self::Linux
        }
    }
}
fn qt_fold_ascii(c: char) -> Option<u8> {
    match c {
        '\u{17f}' => Some(b's'),
        '\u{212a}' => Some(b'k'),
        c if c.is_ascii() => Some(c.to_ascii_lowercase() as u8),
        _ => None,
    }
}
/// Qt CI comparison against ASCII, including long-s/Kelvin folds. The native
/// corpus exhaustively scans Unicode scalars for these non-ASCII mappings.
pub fn qt_ascii_ci_prefix(text: &str, ascii: &str) -> Option<usize> {
    assert!(ascii.is_ascii());
    let mut chars = text.char_indices();
    let mut end = 0;
    for want in ascii.bytes() {
        let (i, c) = chars.next()?;
        if qt_fold_ascii(c)? != want.to_ascii_lowercase() {
            return None;
        }
        end = i + c.len_utf8();
    }
    Some(end)
}
pub fn qt_ascii_ci_suffix(text: &str, ascii: &str) -> bool {
    assert!(ascii.is_ascii());
    let mut chars = text.chars().rev();
    ascii
        .bytes()
        .rev()
        .all(|want| chars.next().and_then(qt_fold_ascii) == Some(want.to_ascii_lowercase()))
}
pub fn file_extension(path: &str) -> &str {
    match path.rfind(['.', '/', '\\']) {
        Some(i) if path.as_bytes()[i] == b'.' => &path[i + 1..],
        _ => "",
    }
}
pub fn name_type_allows_thumbnail(t: NameType) -> bool {
    matches!(
        t,
        NameType::Image
            | NameType::Video
            | NameType::Audio
            | NameType::Document
            | NameType::ThemeFile
    )
}
pub fn file_is_image(name: &str, mime: &str) -> bool {
    if name.is_empty() {
        mime.to_lowercase().starts_with("image/")
    } else {
        detect_name_type(name) == NameType::Image
    }
}
pub fn detect_name_type(path: &str) -> NameType {
    detect_name_type_for(path, FilenamePlatform::current())
}
pub fn detect_name_type_for(path: &str, platform: FilenamePlatform) -> NameType {
    let ext = file_extension(path).to_lowercase();
    let executable = match platform {
        FilenamePlatform::Linux => EXECUTABLE_LINUX,
        FilenamePlatform::Windows => EXECUTABLE_WINDOWS,
        FilenamePlatform::Mac => EXECUTABLE_MAC,
    };
    if executable.split_whitespace().any(|s| s == ext) {
        return NameType::Executable;
    }
    TABLES
        .iter()
        .find_map(|(set, kind)| set.split_whitespace().any(|s| s == ext).then_some(*kind))
        .unwrap_or_default()
}
pub fn sanitize_remote_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if matches!(
                c,
                '\u{200e}'
                    | '\u{200f}'
                    | '\u{202a}'
                    | '\u{202b}'
                    | '\u{202d}'
                    | '\u{202e}'
                    | '\u{2066}'
                    | '\u{2067}'
            ) {
                '_'
            } else {
                c
            }
        })
        .collect()
}
pub fn filename_from_user_string(s: &str, platform: FilenamePlatform) -> String {
    let mut r: String = sanitize_remote_filename(s)
        .chars()
        .map(|c| {
            if c < ' ' || matches!(c, '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    if r.is_empty() || r.ends_with([' ', '.']) {
        r.push('_');
    }
    if platform == FilenamePlatform::Windows {
        let lower = r.to_ascii_lowercase();
        if qt_ascii_ci_suffix(&r, ".lnk") || qt_ascii_ci_suffix(&r, ".scf") {
            r.push_str(".download");
        }
        let stem = lower.split('.').next().unwrap();
        if matches!(stem, "con" | "prn" | "aux" | "nul")
            || (stem.len() == 4
                && (stem.starts_with("com") || stem.starts_with("lpt"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            r.insert(0, '_');
        }
    }
    r
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MimeType {
    pub name: String,
    pub glob_patterns: Vec<String>,
    pub filter_string: String,
}
/// Actual system registry adapter belongs to package 33. Implementations must
/// return registry glob order unchanged: enforceNameType uses the first match.
pub trait MimeDatabase {
    fn for_name(&self, name: &str) -> MimeType;
}
pub fn mime_type_for_name(name: &str, database: &impl MimeDatabase) -> MimeType {
    if let Some((name, glob, label)) = match name {
        "image/webp" => Some(("image/webp", "*.webp", "WebP image")),
        "application/x-tgsticker" => Some(("application/x-tgsticker", "*.tgs", "Telegram sticker")),
        "application/x-tgwallpattern" => {
            Some(("application/x-tgwallpattern", "*.tgv", "Wallpaper pattern"))
        }
        "application/x-tdesktop-theme" | "application/x-tgtheme-tdesktop" => Some((
            "application/x-tdesktop-theme",
            "*.tdesktop-theme",
            "Theme files",
        )),
        "application/x-tdesktop-palette" => Some((
            "application/x-tdesktop-palette",
            "*.tdesktop-palette",
            "Palette files",
        )),
        _ => None,
    } {
        MimeType {
            name: name.into(),
            glob_patterns: vec![glob.into()],
            filter_string: format!("{label} ({glob})"),
        }
    } else {
        database.for_name(if name == "audio/mpeg3" {
            "audio/mp3"
        } else {
            name
        })
    }
}
pub fn is_mime_sticker_lottie(m: &str) -> bool {
    m == "application/x-tgsticker"
}
pub fn is_mime_sticker_webm(m: &str) -> bool {
    m == "video/webm"
}
pub fn is_mime_sticker_animated(m: &str) -> bool {
    is_mime_sticker_lottie(m)
}
pub fn is_mime_sticker(m: &str) -> bool {
    m == "image/webp" || is_mime_sticker_animated(m)
}
pub fn is_mime_sent_as_video(m: &str) -> bool {
    matches!(m, "video/mp4" | "video/quicktime")
}
pub fn is_mime_accepted_for_photo_video_album(m: &str) -> bool {
    matches!(m, "image/jpeg" | "image/png") || is_mime_sent_as_video(m)
}

// Extension sets are transcribed mechanically, retaining platform precedence,
// from the pinned core/mime_type.cpp:257 (see reference tool manifest).
const TABLES: &[(&str, NameType)] = &[
    ("afdesign ai avif bmp dng gif heic icns ico jfif jpeg jpg jpg-large jxl nef png png-large psd qoi raw sketch svg tga tif tiff webp", NameType::Image),
    ("3g2 3gp 3gpp aep avi flv h264 m4s m4v mkv mov mp4 mpeg mpg ogv srt tgs tgv vob webm wmv", NameType::Video),
    ("aac ac3 aif amr caf cda cue flac m4a m4b mid midi mp3 ogg opus wav wma", NameType::Audio),
    ("pdf doc docx ppt pptx pps ppsx xls xlsx txt rtf odt ods odp csv text log tl tex xspf xml djvu diag ps ost kml pub epub mobi cbr cbz fb2 prc ris pem p7b m3u m3u8 wpd wpl htm html xhtml key", NameType::Document),
    ("7z arj bz2 gz rar tar xz z zip zst", NameType::Archive),
    ("tdesktop-theme tdesktop-palette tgios-theme attheme", NameType::ThemeFile),
    ("c cc cpp cxx h m mm swift cs ts class java css ninja cmake patch diff plist gyp gitignore strings asoundrc torrent csr json xaml md keylayout sql sln xib mk dmg img iso vcd pdb eot ics ips ipa core mem pcap ovpn part pcapng dmp pkpass dat zxp crash file bak gbr plain dlc fon fnt otf ttc ttf gpx db rss cur tdesktop-endpoints", NameType::OtherBenign),
];
const EXECUTABLE_WINDOWS: &str = "ad ade adp ahk app application appref-ms asp aspx asx bas bat bin cab cdxml cer cfg cgi chi chm cmd cnt com conf cpl crt csh der diagcab dll drv eml exe fon fxp gadget grp hlp hpj hta htt inf ini ins inx isp isu its jar jnlp job js jse jsp key ksh lexe library-ms lnk local lua mad maf mag mam manifest maq mar mas mat mau mav maw mcf mda mdb mde mdt mdw mdz mht mhtml mjs mmc mof msc msg msh msh1 msh2 msh1xml msh2xml mshxml msi msp mst ops osd paf pcd phar php php3 php4 php5 php7 phps php-s pht phtml pif pl plg pm pod prf prg ps1 ps2 ps1xml ps2xml psc1 psc2 psd1 psm1 pssc pst py py3 pyc pyd pyi pyo pyw pyzw pyz rb reg rgs scf scr sct search-ms settingcontent-ms sh shb shs slk sys swf t tmp u3p url vb vbe vbp vbs vbscript vdx vsmacros vsd vsdm vsdx vss vssm vssx vst vstm vstx vsw vsx vtx website wlua ws wsc wsf wsh xbap xll xlsb xlsm xnk xs";
const EXECUTABLE_MAC: &str = "applescript action app bin command csh osx workflow terminal url caction mpkg pkg scpt scptd xhtm xhtml webarchive";
const EXECUTABLE_LINUX: &str = "bin csh deb desktop ksh out pet pkg pup rpm run sh shar slp zsh";
