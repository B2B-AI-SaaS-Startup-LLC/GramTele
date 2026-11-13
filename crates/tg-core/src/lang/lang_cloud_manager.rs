//! Port of `lang/lang_cloud_manager.{h,cpp}`: language packs from the cloud.
//!
//! What the original reaches through globals is behind [`Delegate`] here:
//! `Core::App()` (restart, capabilities), `Local::` (saving the pack and the
//! recent languages), `Ui::show` / `Ui::hideLayer` (the boxes, described by
//! [`BoxRequest`] and built by the window layer on stage 6) and the file
//! dialog. The two box classes of the `.cpp` (`ConfirmSwitchBox`,
//! `NotReadyBox`) are the widgets of those requests and belong to that layer;
//! their data is in the request.
//!
//! The original subscribes to the active account of `Core::App().domain()` to
//! follow the main MTProto instance. `Main::Domain` is not ported yet, so the
//! owner calls [`CloudManager::set_mtp`] instead, with the same effect.
use super::lang_file_parser::FileParser;
use super::lang_instance::{cloud_lang_pack_name, default_language, Instance, Language, Pack};
use super::lang_keys::{default_language_id, language_id_or_default};
use super::lang_values::PhraseExt;
use super::{get_key_index, get_original_value, tr, KEYS_COUNT};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::{Rc, Weak};
use tg_mtproto::mtp_instance::Instance as MtpInstance;
use tg_mtproto::sender::Sender;
use tg_rpl::{EventStream, Lifetime, Producer};
use tg_scheme::{
    LangPackDifference, LangPackDifferenceData, LangPackLanguage, LangpackGetDifference,
    LangpackGetLangPack, LangpackGetLanguage, LangpackGetLanguages, LangpackGetStrings,
};

/// `ParseLanguage`.
pub fn parse_language(data: &LangPackLanguage) -> Language {
    let LangPackLanguage::LangPackLanguage(data) = data;
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    Language {
        id: text(&data.lang_code),
        plural_id: text(&data.plural_code),
        base_id: text(data.base_lang_code.as_deref().unwrap_or_default()),
        name: text(&data.name),
        native_name: text(&data.native_name),
    }
}

/// A box the original shows with `Ui::show`.
pub enum BoxRequest {
    /// `Ui::MakeInformBox(text)`.
    Inform { text: String },
    /// `Box<ConfirmSwitchBox>(data, apply)`: "switch to this language?" with
    /// its translation progress. The texts are `lng_language_switch_title`,
    /// `lng_language_switch_about_official` / `_unofficial` with `name`,
    /// `percent` and a `lng_language_switch_link` link to `edit_link`;
    /// the buttons are `lng_language_switch_apply` (closes, then `apply`)
    /// and `lng_cancel`.
    ConfirmSwitch {
        name: String,
        percent: i32,
        official: bool,
        edit_link: String,
        apply: Rc<dyn Fn()>,
    },
    /// `Box<NotReadyBox>(data)`: `lng_language_not_ready_title`,
    /// `lng_language_not_ready_about` with `name` and a
    /// `lng_language_not_ready_link` link to `edit_link`; `lng_box_ok`.
    NotReady { name: String, edit_link: String },
    /// `Ui::MakeConfirmBox({ text, confirmed, cancelled, confirmText })`,
    /// shown with `Ui::LayerOption::KeepOther`.
    Confirm {
        text: String,
        confirm_text: String,
        confirmed: Rc<dyn Fn()>,
        cancelled: Option<Rc<dyn Fn()>>,
    },
    /// `Ui::MakeInformBox(text)` shown with `Ui::LayerOption::KeepOther`.
    InformKeepOther { text: String },
}

/// What [`CloudManager`] needs from the application. See the module docs.
pub trait Delegate {
    /// `Core::App().offerLegacyLangPackSwitch()`.
    fn offer_legacy_lang_pack_switch(&self) -> bool;
    /// `Core::App().canApplyLangPackWithoutRestart()`.
    fn can_apply_lang_pack_without_restart(&self) -> bool;
    /// `Core::Restart()`.
    fn restart(&self);
    /// `Local::writeLangPack()`.
    fn write_lang_pack(&self);
    /// `Local::pushRecentLanguage(data)`.
    fn push_recent_language(&self, language: &Language);
    /// `Ui::show(...)`.
    fn show(&self, request: BoxRequest);
    /// `Ui::hideLayer()`.
    fn hide_layer(&self);
    /// `FileDialog::GetOpenPath(parent, title, filter, callback)`: asks for
    /// one file; `callback` gets its path (an empty list of paths in the
    /// original is "no call").
    fn choose_file(&self, title: &str, filter: &str, callback: Box<dyn FnOnce(String)>);
}

/// The callback of `getValueForLang`.
pub type ValueCallback = Rc<dyn Fn(&str)>;

struct ValueRequest {
    request_id: i32,
    callback: ValueCallback,
}

#[derive(Default)]
struct State {
    api: Option<Sender>,
    languages: Vec<Language>,
    lang_pack_request_id: i32,
    lang_pack_base_request_id: i32,
    languages_request_id: i32,

    offer_switch_to_id: String,
    restart_after_switch: bool,

    suggested_language: String,
    language_was_suggested: bool,

    switching_to_language_request: i32,
    switching_to_language_id: String,
    switching_to_language_warning: bool,

    get_keys_for_switch_request_id: i32,

    get_value_for_lang_requests: BTreeMap<String, ValueRequest>,
}

impl State {
    fn pack_request_id(&mut self, pack: Pack) -> &mut i32 {
        if pack != Pack::Base {
            &mut self.lang_pack_request_id
        } else {
            &mut self.lang_pack_base_request_id
        }
    }
}

struct Inner {
    langpack: Instance,
    delegate: Rc<dyn Delegate>,
    state: RefCell<State>,
    language_list_changed: EventStream<()>,
    first_language_suggestion: EventStream<()>,
    lifetime: Lifetime,
}

/// `Lang::CloudManager`.
pub struct CloudManager {
    inner: Rc<Inner>,
}

/// The `this` of a callback: dead once the manager is destroyed, like the
/// `MTP::Sender` requests and the `base::has_weak_ptr` guard of the original.
#[derive(Clone)]
struct WeakManager(Weak<Inner>);

impl WeakManager {
    fn get(&self) -> Option<CloudManager> {
        self.0.upgrade().map(|inner| CloudManager { inner })
    }
}

impl CloudManager {
    pub fn new(langpack: Instance, delegate: Rc<dyn Delegate>) -> Self {
        Self {
            inner: Rc::new(Inner {
                langpack,
                delegate,
                state: RefCell::new(State::default()),
                language_list_changed: EventStream::new(),
                first_language_suggestion: EventStream::new(),
                lifetime: Lifetime::new(),
            }),
        }
    }

    fn weak(&self) -> WeakManager {
        WeakManager(Rc::downgrade(&self.inner))
    }

    fn state(&self) -> std::cell::RefMut<'_, State> {
        self.inner.state.borrow_mut()
    }

    fn has_api(&self) -> bool {
        self.inner.state.borrow().api.is_some()
    }

    /// Cancels a request of ours, `_api->request(id).cancel()`.
    fn cancel(&self, id: i32) {
        if id == 0 {
            return;
        }
        if let Some(api) = &self.inner.state.borrow().api {
            api.cancel(id);
        }
    }

    /// The main MTProto instance appeared (`Some`, the `on_next` of
    /// `mtpMainSessionValue`) or there are no accounts anymore (`None`, the
    /// `on_done` of `activeValue`).
    pub fn set_mtp(&self, instance: Option<MtpInstance>) {
        match instance {
            Some(instance) => {
                self.state().api = Some(Sender::new(instance));
                self.resend_requests();
            }
            None => {
                // Dropping the sender cancels its requests.
                let api = self.state().api.take();
                drop(api);
            }
        }
    }

    /// `packTypeFromId`.
    fn pack_type_from_id(&self, id: &str) -> Pack {
        if id == language_id_or_default(&self.inner.langpack.id()) {
            Pack::Current
        } else if id == self.inner.langpack.base_id() {
            Pack::Base
        } else {
            Pack::None
        }
    }

    /// `languageList`.
    pub fn language_list(&self) -> Vec<Language> {
        self.inner.state.borrow().languages.clone()
    }

    /// `languageListChanged`.
    pub fn language_list_changed(&self) -> Producer<()> {
        self.inner.language_list_changed.events()
    }

    /// `firstLanguageSuggestion`.
    pub fn first_language_suggestion(&self) -> Producer<()> {
        self.inner.first_language_suggestion.events()
    }

    /// `requestLangPackDifference(const QString &langId)`.
    pub fn request_lang_pack_difference_for(&self, lang_id: &str) {
        tg_base::expects!(!lang_id.is_empty());

        if lang_id == language_id_or_default(&self.inner.langpack.id()) {
            self.request_lang_pack_difference(Pack::Current);
        } else {
            self.request_lang_pack_difference(Pack::Base);
        }
    }

    /// `requestLangPackDifference(Pack pack)`.
    fn request_lang_pack_difference(&self, pack: Pack) {
        if !self.has_api() {
            return;
        }
        let previous = std::mem::take(self.state().pack_request_id(pack));
        self.cancel(previous);
        if self.inner.langpack.is_custom() {
            return;
        }

        let version = self.inner.langpack.version(pack);
        let code = self.inner.langpack.cloud_lang_code(pack);
        if code.is_empty() {
            return;
        }
        let done = {
            let weak = self.weak();
            move |result: LangPackDifference| {
                if let Some(this) = weak.get() {
                    *this.state().pack_request_id(pack) = 0;
                    this.apply_lang_pack_difference(&result);
                }
            }
        };
        let fail = {
            let weak = self.weak();
            move |_: &tg_mtproto::mtproto_response::Error| {
                if let Some(this) = weak.get() {
                    *this.state().pack_request_id(pack) = 0;
                }
            }
        };
        let id = {
            let state = self.inner.state.borrow();
            let api = state.api.as_ref().expect("api");
            if version > 0 {
                api.request(LangpackGetDifference::new(
                    cloud_lang_pack_name().into_bytes(),
                    code.into_bytes(),
                    version,
                ))
                .done_local(done)
                .fail_local(fail)
                .send()
            } else {
                api.request(LangpackGetLangPack::new(
                    cloud_lang_pack_name().into_bytes(),
                    code.into_bytes(),
                ))
                .done_local(done)
                .fail_local(fail)
                .send()
            }
        };
        *self.state().pack_request_id(pack) = id;
    }

    /// `setSuggestedLanguage`.
    pub fn set_suggested_language(&self, lang_code: &str) {
        let first = {
            let mut state = self.state();
            state.suggested_language = if language_id_or_default(lang_code) != default_language_id()
            {
                lang_code.to_owned()
            } else {
                String::new()
            };
            !std::mem::replace(&mut state.language_was_suggested, true)
        };
        if !first {
            return;
        }
        self.inner.first_language_suggestion.fire(());

        let suggested = self.inner.state.borrow().suggested_language.clone();
        if self.inner.delegate.offer_legacy_lang_pack_switch()
            && self.inner.langpack.id().is_empty()
            && !suggested.is_empty()
        {
            self.state().offer_switch_to_id = suggested;
            self.offer_switch_lang_pack();
        }
    }

    /// `suggestedLanguage`.
    pub fn suggested_language(&self) -> String {
        self.inner.state.borrow().suggested_language.clone()
    }

    /// `setCurrentVersions`.
    pub fn set_current_versions(&self, version: i32, base_version: i32) {
        let check = |pack: Pack, version: i32| {
            let requesting = *self.state().pack_request_id(pack) != 0;
            if version > self.inner.langpack.version(pack) && !requesting {
                self.request_lang_pack_difference(pack);
            }
        };
        check(Pack::Current, version);
        check(Pack::Base, base_version);
    }

    /// `applyLangPackDifference`.
    pub fn apply_lang_pack_difference(&self, difference: &LangPackDifference) {
        if self.inner.langpack.is_custom() {
            return;
        }

        let LangPackDifference::LangPackDifference(langpack) = difference;
        let langpack_id = String::from_utf8_lossy(&langpack.lang_code).into_owned();
        let pack = self.pack_type_from_id(&langpack_id);
        if pack != Pack::None {
            self.apply_lang_pack_data(pack, langpack);
            let restart = self.inner.state.borrow().restart_after_switch;
            if restart {
                self.restart_after_switch();
            }
        } else {
            tg_base::log!(
                "Lang Warning: Ignoring update for '{}' because our language is '{}'",
                langpack_id,
                self.inner.langpack.id()
            );
        }
    }

    /// `requestLanguageList`.
    pub fn request_language_list(&self) {
        if !self.has_api() {
            self.state().languages_request_id = -1;
            return;
        }
        let previous = std::mem::take(&mut self.state().languages_request_id);
        self.cancel(previous);
        let done = {
            let weak = self.weak();
            move |result: Vec<LangPackLanguage>| {
                let Some(this) = weak.get() else {
                    return;
                };
                let languages: Vec<Language> = result.iter().map(parse_language).collect();
                let changed = {
                    let mut state = this.state();
                    if state.languages != languages {
                        state.languages = languages;
                        true
                    } else {
                        false
                    }
                };
                if changed {
                    this.inner.language_list_changed.fire(());
                }
                this.state().languages_request_id = 0;
            }
        };
        let fail = {
            let weak = self.weak();
            move |_: &tg_mtproto::mtproto_response::Error| {
                if let Some(this) = weak.get() {
                    this.state().languages_request_id = 0;
                }
            }
        };
        let id = {
            let state = self.inner.state.borrow();
            let api = state.api.as_ref().expect("api");
            api.request(LangpackGetLanguages::new(
                cloud_lang_pack_name().into_bytes(),
            ))
            .done_local(done)
            .fail_local(fail)
            .send()
        };
        self.state().languages_request_id = id;
    }

    /// `offerSwitchLangPack`.
    fn offer_switch_lang_pack(&self) {
        {
            let state = self.inner.state.borrow();
            tg_base::expects!(!state.offer_switch_to_id.is_empty());
            tg_base::expects!(state.offer_switch_to_id != default_language_id());
        }

        if !self.show_offer_switch_box() {
            let weak = self.weak();
            self.language_list_changed().on_next(
                move |_| {
                    if let Some(this) = weak.get() {
                        this.show_offer_switch_box();
                    }
                },
                &self.inner.lifetime,
            );
            self.request_language_list();
        }
    }

    /// `findOfferedLanguage`.
    fn find_offered_language(&self) -> Language {
        let state = self.inner.state.borrow();
        state
            .languages
            .iter()
            .find(|language| language.id == state.offer_switch_to_id)
            .cloned()
            .unwrap_or_default()
    }

    /// `showOfferSwitchBox`.
    fn show_offer_switch_box(&self) -> bool {
        let language = self.find_offered_language();
        if language.id.is_empty() {
            return false;
        }

        let confirm = {
            let weak = self.weak();
            let language = language.clone();
            move || {
                let Some(this) = weak.get() else {
                    return;
                };
                this.inner.delegate.hide_layer();
                if this.inner.state.borrow().offer_switch_to_id.is_empty() {
                    return;
                }
                this.perform_switch_and_restart(&language);
            }
        };
        let cancel = {
            let weak = self.weak();
            move || {
                let Some(this) = weak.get() else {
                    return;
                };
                this.inner.delegate.hide_layer();
                this.change_id_and_re_init_connection(&default_language());
                this.inner.delegate.write_lang_pack();
            }
        };
        self.inner.delegate.show(BoxRequest::Confirm {
            text: format!(
                "Do you want to switch your language to {}? You can always change your language in Settings.",
                language.native_name
            ),
            confirm_text: "Change".into(),
            confirmed: Rc::new(confirm),
            cancelled: Some(Rc::new(cancel)),
        });
        true
    }

    /// `applyLangPackData`.
    fn apply_lang_pack_data(&self, pack: Pack, data: &LangPackDifferenceData) {
        let restart = self.inner.state.borrow().restart_after_switch;
        if self.inner.langpack.version(pack) < data.from_version {
            self.request_lang_pack_difference(pack);
        } else if !data.strings.is_empty() {
            self.inner.langpack.apply_difference(pack, data);
            self.inner.delegate.write_lang_pack();
        } else if restart {
            self.inner.delegate.write_lang_pack();
        } else {
            tg_base::log!("Lang Info: Up to date.");
        }
    }

    /// `canApplyWithoutRestart`.
    fn can_apply_without_restart(&self, id: &str) -> bool {
        if id == "#TEST_X" || id == "#TEST_0" {
            return true;
        }
        self.inner.delegate.can_apply_lang_pack_without_restart()
    }

    /// `resetToDefault`.
    pub fn reset_to_default(&self) {
        self.perform_switch(&default_language());
    }

    /// `switchToLanguage(const QString &id)`.
    pub fn switch_to_language_id(&self, id: &str) {
        self.request_language_and_switch(id, false);
    }

    /// `switchWithWarning`.
    pub fn switch_with_warning(&self, id: &str) {
        self.request_language_and_switch(id, true);
    }

    /// `requestLanguageAndSwitch`.
    fn request_language_and_switch(&self, id: &str, warning: bool) {
        tg_base::expects!(!id.is_empty());

        if language_id_or_default(&self.inner.langpack.id()) == id {
            self.inner.delegate.show(BoxRequest::Inform {
                text: tr::lng_language_already.now(),
            });
            return;
        } else if id == "#custom" {
            self.perform_switch_to_custom();
            return;
        }

        {
            let mut state = self.state();
            state.switching_to_language_id = id.to_owned();
            state.switching_to_language_warning = warning;
        }
        self.send_switching_to_language_request();
    }

    /// `sendSwitchingToLanguageRequest`.
    fn send_switching_to_language_request(&self) {
        if !self.has_api() {
            self.state().switching_to_language_request = -1;
            return;
        }
        let previous = self.inner.state.borrow().switching_to_language_request;
        self.cancel(previous);
        let done = {
            let weak = self.weak();
            move |result: LangPackLanguage| {
                let Some(this) = weak.get() else {
                    return;
                };
                this.state().switching_to_language_request = 0;
                let language = parse_language(&result);
                let finalize: Rc<dyn Fn()> = {
                    let weak = this.weak();
                    Rc::new(move || {
                        let Some(this) = weak.get() else {
                            return;
                        };
                        if this.can_apply_without_restart(&language.id) {
                            this.perform_switch_and_add_to_recent(&language);
                        } else {
                            this.perform_switch_and_restart(&language);
                        }
                    })
                };
                if !this.inner.state.borrow().switching_to_language_warning {
                    finalize();
                    return;
                }
                let LangPackLanguage::LangPackLanguage(data) = &result;
                let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
                if data.strings_count > 0 {
                    this.inner.delegate.show(BoxRequest::ConfirmSwitch {
                        name: text(&data.native_name),
                        percent: data.translated_count * 100 / data.strings_count,
                        official: data
                            .flags
                            .contains(tg_scheme::LangPackLanguageData::F_OFFICIAL),
                        edit_link: text(&data.translations_url),
                        apply: finalize,
                    });
                } else {
                    this.inner.delegate.show(BoxRequest::NotReady {
                        name: text(&data.native_name),
                        edit_link: text(&data.translations_url),
                    });
                }
            }
        };
        let fail = {
            let weak = self.weak();
            move |error: &tg_mtproto::mtproto_response::Error| {
                let Some(this) = weak.get() else {
                    return;
                };
                this.state().switching_to_language_request = 0;
                if error.kind == "LANG_CODE_NOT_SUPPORTED" {
                    this.inner.delegate.show(BoxRequest::Inform {
                        text: tr::lng_language_not_found.now(),
                    });
                }
            }
        };
        let id = {
            let state = self.inner.state.borrow();
            let api = state.api.as_ref().expect("api");
            api.request(LangpackGetLanguage::new(
                cloud_lang_pack_name().into_bytes(),
                state.switching_to_language_id.clone().into_bytes(),
            ))
            .done_local(done)
            .fail_local(fail)
            .send()
        };
        self.state().switching_to_language_request = id;
    }

    /// `switchToLanguage(const Language &data)`.
    pub fn switch_to_language(&self, data: &Language) {
        if (self.inner.langpack.id() == data.id && data.id != "#custom") || !self.has_api() {
            return;
        }

        let previous = std::mem::take(&mut self.state().get_keys_for_switch_request_id);
        self.cancel(previous);
        if data.id == "#custom" {
            self.perform_switch_to_custom();
        } else if self.can_apply_without_restart(&data.id) {
            self.perform_switch_and_add_to_recent(data);
        } else {
            let done = {
                let weak = self.weak();
                let data = data.clone();
                move |result: Vec<tg_scheme::LangPackString>| {
                    let Some(this) = weak.get() else {
                        return;
                    };
                    this.state().get_keys_for_switch_request_id = 0;
                    let values = Instance::parse_strings(&result);
                    let key = tr::lng_sure_save_language.base;
                    let translated = values
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| get_original_value(key));
                    let text = format!("{}\n\n{}", tr::lng_sure_save_language.now(), translated);
                    let weak = this.weak();
                    let data = data.clone();
                    this.inner.delegate.show(BoxRequest::Confirm {
                        text,
                        confirm_text: tr::lng_box_ok.now(),
                        confirmed: Rc::new(move || {
                            if let Some(this) = weak.get() {
                                this.perform_switch_and_restart(&data);
                            }
                        }),
                        cancelled: None,
                    });
                }
            };
            let fail = {
                let weak = self.weak();
                move |_: &tg_mtproto::mtproto_response::Error| {
                    if let Some(this) = weak.get() {
                        this.state().get_keys_for_switch_request_id = 0;
                    }
                }
            };
            let id = {
                let state = self.inner.state.borrow();
                let api = state.api.as_ref().expect("api");
                api.request(LangpackGetStrings::new(
                    cloud_lang_pack_name().into_bytes(),
                    data.id.clone().into_bytes(),
                    vec![b"lng_sure_save_language".to_vec()],
                ))
                .done_local(done)
                .fail_local(fail)
                .send()
            };
            self.state().get_keys_for_switch_request_id = id;
        }
    }

    /// `performSwitchToCustom`.
    fn perform_switch_to_custom(&self) {
        let filter = "Language files (*.strings)";
        let title = "Choose language .strings file";
        let weak = self.weak();
        let callback = move |file_path: String| {
            let Some(this) = weak.get() else {
                return;
            };
            let key = tr::lng_sure_save_language.base;
            let loader = FileParser::new(&FileParser::read_file(
                std::path::Path::new(&file_path),
                std::path::Path::new(&file_path),
            ));
            if loader.errors().is_empty() {
                let previous = std::mem::take(&mut this.state().switching_to_language_request);
                this.cancel(previous);
                if this.can_apply_without_restart("#custom") {
                    this.inner.langpack.switch_to_custom_file(&file_path);
                } else {
                    let request = BTreeSet::from([key]);
                    let values = loader.found(
                        |name| Some(get_key_index(name)).filter(|index| *index != KEYS_COUNT),
                        &request,
                    );
                    let translated = values
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| get_original_value(key));
                    let text = format!("{}\n\n{}", tr::lng_sure_save_language.now(), translated);
                    let weak = this.weak();
                    let change = move || {
                        if let Some(this) = weak.get() {
                            this.inner.langpack.switch_to_custom_file(&file_path);
                            this.inner.delegate.restart();
                        }
                    };
                    this.inner.delegate.show(BoxRequest::Confirm {
                        text,
                        confirm_text: tr::lng_box_ok.now(),
                        confirmed: Rc::new(change),
                        cancelled: None,
                    });
                }
            } else {
                this.inner.delegate.show(BoxRequest::InformKeepOther {
                    text: format!("Custom lang failed :(\n\nError: {}", loader.errors()),
                });
            }
        };
        self.inner
            .delegate
            .choose_file(title, filter, Box::new(callback));
    }

    /// `switchToTestLanguage`.
    pub fn switch_to_test_language(&self) {
        let test_language_id = if self.inner.langpack.id() == "#TEST_X" {
            "#TEST_0"
        } else {
            "#TEST_X"
        };
        self.perform_switch(&Language {
            id: test_language_id.into(),
            ..Default::default()
        });
    }

    /// `performSwitch`.
    fn perform_switch(&self, data: &Language) {
        self.state().restart_after_switch = false;
        self.switch_lang_pack_id(data);
        self.request_lang_pack_difference(Pack::Current);
        self.request_lang_pack_difference(Pack::Base);
    }

    /// `performSwitchAndAddToRecent`.
    fn perform_switch_and_add_to_recent(&self, data: &Language) {
        self.inner.delegate.push_recent_language(data);
        self.perform_switch(data);
    }

    /// `performSwitchAndRestart`.
    fn perform_switch_and_restart(&self, data: &Language) {
        self.perform_switch_and_add_to_recent(data);
        self.restart_after_switch();
    }

    /// `restartAfterSwitch`.
    fn restart_after_switch(&self) {
        let requesting = {
            let state = self.inner.state.borrow();
            state.lang_pack_request_id != 0 || state.lang_pack_base_request_id != 0
        };
        if requesting {
            self.state().restart_after_switch = true;
        } else {
            self.inner.delegate.restart();
        }
    }

    /// `switchLangPackId`.
    fn switch_lang_pack_id(&self, data: &Language) {
        let current_id = self.inner.langpack.id();
        let current_base_id = self.inner.langpack.base_id();
        let not_changed = (current_id == data.id && current_base_id == data.base_id)
            || (current_id.is_empty()
                && current_base_id.is_empty()
                && data.id == default_language_id());
        if !not_changed {
            self.change_id_and_re_init_connection(data);
        }
    }

    /// `changeIdAndReInitConnection`.
    fn change_id_and_re_init_connection(&self, data: &Language) {
        self.inner.langpack.switch_to_id(data);
        let mtproto = self
            .inner
            .state
            .borrow()
            .api
            .as_ref()
            .map(|api| api.instance().clone());
        if let Some(mtproto) = mtproto {
            mtproto.re_init_connection(mtproto.main_dc_id());
        }
    }

    /// `getValueForLang`: the value of one key in another language.
    pub fn get_value_for_lang(&self, key: &str, lang_id: &str, callback: ValueCallback) {
        let request_key = format!("{lang_id}:{key}");
        let previous = {
            let mut state = self.state();
            let request = state
                .get_value_for_lang_requests
                .entry(request_key.clone())
                .or_insert_with(|| ValueRequest {
                    request_id: 0,
                    callback: callback.clone(),
                });
            request.callback = callback;
            if state.api.is_none() {
                state
                    .get_value_for_lang_requests
                    .get_mut(&request_key)
                    .expect("request")
                    .request_id = -1;
                return;
            }
            let request = state
                .get_value_for_lang_requests
                .get_mut(&request_key)
                .expect("request");
            std::mem::take(&mut request.request_id)
        };
        self.cancel(previous);
        let take_callback = {
            let weak = self.weak();
            let request_key = request_key.clone();
            move || {
                let this = weak.get()?;
                let request = this
                    .state()
                    .get_value_for_lang_requests
                    .remove(&request_key)?;
                Some(request.callback)
            }
        };
        let done = {
            let take_callback = take_callback.clone();
            move |result: Vec<tg_scheme::LangPackString>| {
                if let Some(onstack) = take_callback() {
                    let values = Instance::parse_strings(&result);
                    match values.into_values().next() {
                        Some(value) => onstack(&value),
                        None => onstack(""),
                    }
                }
            }
        };
        let fail = move |_: &tg_mtproto::mtproto_response::Error| {
            if let Some(onstack) = take_callback() {
                onstack("");
            }
        };
        let id = {
            let state = self.inner.state.borrow();
            let api = state.api.as_ref().expect("api");
            api.request(LangpackGetStrings::new(
                cloud_lang_pack_name().into_bytes(),
                lang_id.as_bytes().to_vec(),
                vec![key.as_bytes().to_vec()],
            ))
            .done_local(done)
            .fail_local(fail)
            .send()
        };
        if let Some(request) = self
            .state()
            .get_value_for_lang_requests
            .get_mut(&request_key)
        {
            request.request_id = id;
        }
    }

    /// `resendPendingValueRequests`.
    fn resend_pending_value_requests(&self) {
        if !self.has_api() {
            return;
        }
        let pending: Vec<(String, ValueCallback)> = self
            .inner
            .state
            .borrow()
            .get_value_for_lang_requests
            .iter()
            .filter(|(_, request)| request.request_id == -1)
            .map(|(key, request)| (key.clone(), request.callback.clone()))
            .collect();
        for (request_key, callback) in pending {
            if let Some((lang_id, key)) = request_key.split_once(':') {
                // `colonPos > 0`: a request with an empty language is not
                // sent again.
                if !lang_id.is_empty() {
                    self.get_value_for_lang(key, lang_id, callback);
                }
            }
        }
    }

    /// `resendRequests`.
    fn resend_requests(&self) {
        let (base, current, languages, switching) = {
            let state = self.inner.state.borrow();
            (
                state.lang_pack_base_request_id != 0,
                state.lang_pack_request_id != 0,
                state.languages_request_id != 0,
                state.switching_to_language_request != 0,
            )
        };
        if base {
            self.request_lang_pack_difference(Pack::Base);
        }
        if current {
            self.request_lang_pack_difference(Pack::Current);
        }
        if languages {
            self.request_language_list();
        }
        if switching {
            self.send_switching_to_language_request();
        }
        self.resend_pending_value_requests();
    }
}
