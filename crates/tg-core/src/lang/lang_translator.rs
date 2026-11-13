//! Port of `lang/lang_translator.{h,cpp}`.
//!
//! `Lang::Translator` is a `QTranslator` in the original: Qt asks it for the
//! texts of its own menus. There is no Qt in the port, so this is the lookup
//! alone; the window layer (stage 5) calls it for the standard menus it
//! builds itself. `disambiguation` and `n` are not used by the original.
use super::lang_values::PhraseExt;
use super::{lt_telegram, tr};

/// The platform whose menu wording is used; `Platform::IsWindows()` /
/// `Platform::IsMac()` in the original.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuPlatform {
    Windows,
    Mac,
    Linux,
}

impl MenuPlatform {
    /// The platform this build runs on.
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

/// `Translator::translate`; an empty string means "not translated".
pub fn translate(context: &str, source_text: &str, platform: MenuPlatform) -> String {
    if context == "QMenuBar" {
        return match source_text {
            "Services" => tr::lng_mac_menu_services.now(),
            "Hide %1" => tr::lng_mac_menu_hide_telegram.tag(lt_telegram, "%1").now(),
            "Hide Others" => tr::lng_mac_menu_hide_others.now(),
            "Show All" => tr::lng_mac_menu_show_all.now(),
            "Preferences..." => tr::lng_mac_menu_preferences.now(),
            "Quit %1" => tr::lng_mac_menu_quit_telegram.tag(lt_telegram, "%1").now(),
            "About %1" => tr::lng_mac_menu_about_telegram.tag(lt_telegram, "%1").now(),
            _ => String::new(),
        };
    }
    if context == "QWidgetTextControl" || context == "QLineEdit" {
        return match source_text {
            "&Undo" => match platform {
                MenuPlatform::Windows => tr::lng_wnd_menu_undo.now(),
                MenuPlatform::Mac => tr::lng_mac_menu_undo.now(),
                MenuPlatform::Linux => tr::lng_linux_menu_undo.now(),
            },
            "&Redo" => match platform {
                MenuPlatform::Windows => tr::lng_wnd_menu_redo.now(),
                MenuPlatform::Mac => tr::lng_mac_menu_redo.now(),
                MenuPlatform::Linux => tr::lng_linux_menu_redo.now(),
            },
            "Cu&t" => tr::lng_mac_menu_cut.now(),
            "&Copy" => tr::lng_mac_menu_copy.now(),
            "&Paste" => tr::lng_mac_menu_paste.now(),
            "Delete" => tr::lng_mac_menu_delete.now(),
            "Select All" => tr::lng_mac_menu_select_all.now(),
            _ => String::new(),
        };
    }
    if context == "QUnicodeControlCharacterMenu" {
        return match source_text {
            "Insert Unicode control character" => tr::lng_menu_insert_unicode.now(),
            _ => String::new(),
        };
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::lang_instance::{instance, Language};

    #[test]
    fn qt_menu_texts() {
        instance().switch_to_id(&Language {
            id: "en".into(),
            ..Default::default()
        });
        let linux = MenuPlatform::Linux;
        assert_eq!(translate("QMenuBar", "Services", linux), "Services");
        assert_eq!(
            translate("QMenuBar", "Hide %1", linux),
            tr::lng_mac_menu_hide_telegram.tag(lt_telegram, "%1").now()
        );
        assert!(translate("QMenuBar", "Quit %1", linux).contains("%1"));
        assert!(translate("QMenuBar", "About %1", linux).contains("%1"));
        assert!(!translate("QMenuBar", "Hide Others", linux).is_empty());
        assert!(!translate("QMenuBar", "Show All", linux).is_empty());
        assert!(!translate("QMenuBar", "Preferences...", linux).is_empty());
        assert_eq!(translate("QMenuBar", "Other", linux), "");

        assert_eq!(translate("QLineEdit", "&Undo", linux), "Undo");
        assert_eq!(
            translate("QWidgetTextControl", "&Undo", MenuPlatform::Windows),
            tr::lng_wnd_menu_undo.now()
        );
        assert_eq!(
            translate("QLineEdit", "&Redo", MenuPlatform::Mac),
            tr::lng_mac_menu_redo.now()
        );
        assert_eq!(
            translate("QLineEdit", "&Redo", MenuPlatform::Windows),
            tr::lng_wnd_menu_redo.now()
        );
        assert_eq!(
            translate("QLineEdit", "&Undo", MenuPlatform::Mac),
            tr::lng_mac_menu_undo.now()
        );
        assert_eq!(
            translate("QLineEdit", "&Redo", linux),
            tr::lng_linux_menu_redo.now()
        );
        for source in ["Cu&t", "&Copy", "&Paste", "Delete", "Select All"] {
            assert!(
                !translate("QLineEdit", source, linux).is_empty(),
                "{source}"
            );
        }
        assert_eq!(translate("QLineEdit", "Other", linux), "");
        assert_eq!(
            translate(
                "QUnicodeControlCharacterMenu",
                "Insert Unicode control character",
                linux
            ),
            "Insert Unicode control character"
        );
        assert_eq!(
            translate("QUnicodeControlCharacterMenu", "Other", linux),
            ""
        );
        assert_eq!(translate("QDialog", "Services", linux), "");
        let _ = MenuPlatform::current();
    }
}
