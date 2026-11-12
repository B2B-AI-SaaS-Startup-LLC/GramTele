//! Сохраняемые значения AyuGram; GUI-побочные эффекты возвращаются владельцу.
use serde_json::{json, Map, Value};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};
use tg_rpl::{Producer, Variable};
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Bool,
    Text,
    Float,
    Int,
    U32,
    U64,
    Enum,
    Translation,
    Ids,
}
pub struct Spec {
    pub key: &'static str,
    pub kind: Kind,
    pub default_json: &'static str,
}
include!("ayu_settings_specs.rs");
#[derive(Debug)]
pub enum Error {
    Json(serde_json::Error),
    Io(std::io::Error),
    Type(String),
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
fn normalize(spec: &Spec, v: &Value) -> Result<Value, Error> {
    let bad = || Error::Type(spec.key.into());
    Ok(match spec.kind {
        Kind::Bool => Value::Bool(v.as_bool().ok_or_else(bad)?),
        Kind::Text => Value::String(v.as_str().ok_or_else(bad)?.into()),
        Kind::Float => json!(v.as_f64().ok_or_else(bad)?),
        Kind::Int | Kind::U32 | Kind::U64 => {
            if !v.is_number() && !v.is_boolean() {
                return Err(bad());
            }
            let n = if let Some(n) = v.as_bool() {
                u64::from(n)
            } else if let Some(n) = v.as_i64() {
                n as u64
            } else if let Some(n) = v.as_u64() {
                n
            } else {
                let number = v.as_f64().ok_or_else(bad)?;
                if matches!(spec.kind, Kind::U64) {
                    number as u64
                } else {
                    number as i64 as u64
                }
            };
            match spec.kind {
                Kind::Int => json!(n as i32),
                Kind::U32 => json!(n as u32),
                _ => json!(n),
            }
        }
        Kind::Enum => {
            let n = v
                .as_f64()
                .filter(|n| [0., 1., 2.].contains(n))
                .unwrap_or(0.);
            json!(n as i32)
        }
        Kind::Translation => json!(v
            .as_str()
            .filter(|s| ["telegram", "google", "yandex", "native"].contains(s))
            .unwrap_or("telegram")),
        Kind::Ids => {
            let a = v.as_array().ok_or_else(bad)?;
            let mut ids = Vec::new();
            for n in a {
                if !n.is_number() && !n.is_boolean() {
                    return Err(bad());
                }
                let n = n
                    .as_bool()
                    .map(i64::from)
                    .or_else(|| n.as_i64())
                    .unwrap_or_else(|| {
                        n.as_u64()
                            .map(|n| n as i64)
                            .unwrap_or_else(|| n.as_f64().unwrap() as i64)
                    });
                ids.push(n);
            }
            json!(crate::json_ext::set_from_iter(ids).0)
        }
    })
}
pub struct Section {
    specs: &'static [Spec],
    values: BTreeMap<&'static str, Variable<Value>>,
    ids_buckets: std::cell::Cell<usize>,
}
impl Section {
    pub fn new(specs: &'static [Spec]) -> Self {
        Self {
            specs,
            ids_buckets: std::cell::Cell::new(0),
            values: specs
                .iter()
                .map(|s| {
                    (
                        s.key,
                        Variable::new(serde_json::from_str(s.default_json).unwrap()),
                    )
                })
                .collect(),
        }
    }
    pub fn get(&self, key: &str) -> Option<Value> {
        self.values.get(key).map(Variable::current)
    }
    pub fn value(&self, key: &str) -> Option<Producer<Value>> {
        if key == "shadowBanIds" {
            return None;
        }
        self.values.get(key).map(Variable::value)
    }
    pub fn changes(&self, key: &str) -> Option<Producer<Value>> {
        if key == "shadowBanIds" {
            return None;
        }
        self.values.get(key).map(Variable::changes)
    }
    fn assign(&self, key: &str, v: Value) -> bool {
        let var = &self.values[key];
        let changed = var.current() != v;
        if key == "shadowBanIds" {
            self.ids_buckets.set(0);
        }
        var.set(v);
        changed
    }
    #[allow(clippy::wrong_self_convention)] // original from_json mutates partially before failure
    fn from_json(&self, j: &Value) -> Result<(), Error> {
        self.read_json(j, None)
    }
    fn read_json(&self, j: &Value, edited_mark: Option<&str>) -> Result<(), Error> {
        if !j.is_object() {
            return Err(Error::Type("expected object".into()));
        }
        for spec in self.specs {
            let legacy = match spec.key {
                "embeddedThemeType" => Some("themeType"),
                "embeddedThemeAccentColor" => Some("themeAccentColor"),
                _ => None,
            };
            let default = if let Some(key) = legacy {
                match j.get(key) {
                    Some(v) => normalize(spec, v)?,
                    None => serde_json::from_str(spec.default_json)?,
                }
            } else {
                serde_json::from_str(spec.default_json)?
            };
            let v = match j.get(spec.key) {
                Some(v) if spec.key == "sendWithoutSound" && v.is_boolean() => {
                    json!(if v == true { 2 } else { 0 })
                }
                Some(v) => normalize(spec, v)?,
                None if spec.key == "editedMark" && edited_mark.is_some() => {
                    json!(edited_mark.unwrap())
                }
                None => default,
            };
            self.assign(spec.key, v);
            if spec.key == "shadowBanIds" {
                self.ids_buckets.set(
                    j.get(spec.key)
                        .and_then(Value::as_array)
                        .map_or(0, |a| crate::json_ext::reserve_buckets(a.len())),
                );
            }
        }
        Ok(())
    }
    fn json(&self) -> Value {
        Value::Object(
            self.values
                .iter()
                .map(|(k, v)| ((*k).into(), v.current()))
                .collect(),
        )
    }
    pub fn ghost_mode_active(&self) -> bool {
        [
            "sendReadMessages",
            "sendReadStories",
            "sendOnlinePackets",
            "sendUploadProgress",
        ]
        .iter()
        .all(|key| {
            self.values[&*format!("{key}Locked")].current() == true
                || self.values[key].current() == false
        }) && (self.values["sendOfflinePacketAfterOnlineLocked"].current() == true
            || self.values["sendOfflinePacketAfterOnline"].current() == true)
    }
    pub fn ghost_mode_active_value(&self) -> Producer<bool> {
        let streams = self
            .values
            .values()
            .map(Variable::value)
            .collect::<Vec<_>>();
        let keys = self.values.keys().copied().collect::<Vec<_>>();
        tg_rpl::combine_vec(streams)
            .map(move |values| {
                let v: BTreeMap<_, _> = keys.iter().copied().zip(values).collect();
                [
                    "sendReadMessages",
                    "sendReadStories",
                    "sendOnlinePackets",
                    "sendUploadProgress",
                ]
                .iter()
                .all(|k| v[&*format!("{k}Locked")] == true || v[k] == false)
                    && (v["sendOfflinePacketAfterOnlineLocked"] == true
                        || v["sendOfflinePacketAfterOnline"] == true)
            })
            .distinct_until_changed()
    }
    pub fn should_send_without_sound(&self) -> bool {
        match self.values["sendWithoutSound"].current().as_i64().unwrap() {
            0 => false,
            1 => self.ghost_mode_active(),
            2 => true,
            _ => unreachable!(),
        }
    }
    pub fn is_use_scheduled_messages(&self) -> bool {
        self.ghost_mode_active() && self.values["useScheduledMessages"].current() == true
    }
    pub fn is_cloud_theme_empty(&self) -> bool {
        [
            "cloudThemeId",
            "cloudThemeAccessHash",
            "cloudThemeDocumentId",
        ]
        .iter()
        .all(|k| self.values[k].current() == 0)
            && self.values["cloudThemeTitle"].current() == ""
    }
}
pub struct GhostModeAccountSettings(Section);
impl Default for GhostModeAccountSettings {
    fn default() -> Self {
        Self(Section::new(GHOST_MODE_ACCOUNT_SETTINGS))
    }
}
impl std::ops::Deref for GhostModeAccountSettings {
    type Target = Section;
    fn deref(&self) -> &Section {
        &self.0
    }
}
impl GhostModeAccountSettings {
    pub fn is_ghost_mode_active(&self) -> bool {
        self.ghost_mode_active()
    }
    pub fn ghost_mode_active(&self) -> bool {
        self.0.ghost_mode_active()
    }
    pub fn ghost_mode_active_value(&self) -> Producer<bool> {
        self.0.ghost_mode_active_value()
    }
    pub fn ghost_mode_active_changes(&self) -> Producer<bool> {
        self.ghost_mode_active_value().skip(1)
    }
    pub fn should_send_without_sound(&self) -> bool {
        self.0.should_send_without_sound()
    }
    pub fn is_use_scheduled_messages(&self) -> bool {
        self.0.is_use_scheduled_messages()
    }
}
pub struct MessageShotSettings(Section);
impl Default for MessageShotSettings {
    fn default() -> Self {
        Self(Section::new(MESSAGE_SHOT_SETTINGS))
    }
}
impl std::ops::Deref for MessageShotSettings {
    type Target = Section;
    fn deref(&self) -> &Section {
        &self.0
    }
}
impl MessageShotSettings {
    pub fn is_cloud_theme_empty(&self) -> bool {
        self.0.is_cloud_theme_empty()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Repaint,
    MaterialSwitches(bool),
    AvatarCorners(i32),
    RefreshUnreadBadge,
    ResetTranslationCache,
    StreamerMode(bool),
    RebuildFilters,
    MarkAsOnline(u64),
}
pub struct AyuSettings {
    pub settings: Section,
    pub message_shot: MessageShotSettings,
    ghost_accounts: BTreeMap<u64, GhostModeAccountSettings>,
    path: Option<PathBuf>,
    pub native_translation_available: bool,
    default_edited_mark: String,
    effects: tg_rpl::EventStream<Effect>,
}
impl Default for AyuSettings {
    fn default() -> Self {
        Self {
            settings: Section::new(AYU_SETTINGS),
            message_shot: MessageShotSettings::default(),
            ghost_accounts: BTreeMap::new(),
            path: None,
            native_translation_available: false,
            default_edited_mark: "edited".into(),
            effects: tg_rpl::EventStream::new(),
        }
    }
}
impl AyuSettings {
    /// Полученные от ядра локализованное «edited» и доступность Native-переводчика.
    pub fn with_defaults(edited_mark: String, native_translation_available: bool) -> Self {
        let result = Self {
            default_edited_mark: edited_mark.clone(),
            native_translation_available,
            ..Self::default()
        };
        result.settings.assign("editedMark", json!(edited_mark));
        result
    }
    pub fn effects(&self) -> Producer<Effect> {
        self.effects.events()
    }
    fn emit_effects(&self, effects: &[Effect]) {
        for effect in effects {
            self.effects.fire(effect.clone());
        }
    }

    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            path: Some(path.into()),
            ..Self::default()
        }
    }
    pub fn from_json(&mut self, j: &Value) -> Result<(), Error> {
        if let Some(accounts) = j.get("ghostModeSettings").and_then(Value::as_object) {
            self.ghost_accounts.clear();
            let mut accounts = accounts.iter().collect::<Vec<_>>();
            accounts.sort_by_key(|(key, _)| *key);
            for (key, v) in accounts {
                let account = GhostModeAccountSettings::default();
                account.from_json(v)?;
                self.ghost_accounts.insert(stoull(key)?, account);
            }
        }
        self.settings
            .read_json(j, Some(&self.default_edited_mark))?;
        if let Some(shot) = j.get("messageShotSettings").filter(|v| v.is_object()) {
            self.message_shot.from_json(shot)?;
        }
        Ok(())
    }
    pub fn to_json(&self) -> Value {
        let mut j = self.settings.json();
        j["ghostModeSettings"] = Value::Object(
            self.ghost_accounts
                .iter()
                .map(|(id, s)| (id.to_string(), s.json()))
                .collect(),
        );
        j["messageShotSettings"] = self.message_shot.json();
        j
    }
    pub fn json_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
        let mut serializer = serde_json::Serializer::with_formatter(&mut out, formatter);
        let mut value = self.to_json();
        value.sort_all_objects();
        serde::Serialize::serialize(&value, &mut serializer)?;
        let value = self
            .settings
            .get("wideMultiplier")
            .unwrap()
            .as_f64()
            .unwrap();
        let text = String::from_utf8(out).unwrap();
        let marker = "\"wideMultiplier\": ";
        let start = text.rfind(marker).unwrap() + marker.len();
        let end = text[start..]
            .find([',', '\n'])
            .map_or(text.len(), |n| start + n);
        let text = format!(
            "{}{}{}",
            &text[..start],
            crate::json_ext::format_double(value),
            &text[end..]
        );
        Ok(text.into_bytes())
    }
    pub fn save(&self) -> Result<(), Error> {
        if let Some(path) = &self.path {
            let mut f = std::fs::File::create(path)?;
            f.write_all(&self.json_bytes()?)?;
        }
        Ok(())
    }
    pub fn load(
        &mut self,
        command_line_ghost: bool,
        active_account: Option<u64>,
    ) -> Result<(), Error> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        // operator>> consumes one JSON value; trailing bytes do not invalidate it.
        let parse = serde_json::Deserializer::from_slice(
            bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes),
        )
        .into_iter::<Value>()
        .next()
        .transpose();
        let result = match parse {
            Ok(Some(mut j)) => {
                let migration = migrate_ghost(&mut j);
                migration.and_then(|()| self.from_json(&j))
            }
            Ok(None) => Err(Error::Type("empty JSON".into())),
            Err(e) => Err(e.into()),
        };
        if command_line_ghost {
            let g = self.ghost(active_account.unwrap_or(0));
            for key in [
                "sendReadMessages",
                "sendReadStories",
                "sendOnlinePackets",
                "sendUploadProgress",
            ] {
                g.assign(key, json!(false));
            }
            g.assign("sendOfflinePacketAfterOnline", json!(true));
        }
        self.validate()?;
        if let Err(error) = &result {
            tg_base::log!("AyuGramSettings: failed to parse settings file: {error}");
        }
        result
    }
    pub fn reset(&mut self) -> Result<(), Error> {
        for spec in AYU_SETTINGS {
            let value = if spec.key == "editedMark" {
                json!(self.default_edited_mark)
            } else {
                serde_json::from_str(spec.default_json)?
            };
            self.settings.assign(spec.key, value);
        }
        for spec in MESSAGE_SHOT_SETTINGS {
            self.message_shot
                .assign(spec.key, serde_json::from_str(spec.default_json)?);
        }
        self.ghost_accounts.clear();
        self.save()
    }
    pub fn ghost(&mut self, user: u64) -> &GhostModeAccountSettings {
        let id = if self.settings.get("useGlobalGhostMode") == Some(json!(true)) {
            0
        } else {
            user
        };
        self.ghost_accounts.entry(id).or_default()
    }
    pub fn set(&mut self, key: &str, value: Value) -> Result<Vec<Effect>, Error> {
        let spec = AYU_SETTINGS
            .iter()
            .find(|s| s.key == key)
            .ok_or_else(|| Error::Type(key.into()))?;
        let ids_buckets = value
            .as_array()
            .map(|a| crate::json_ext::reserve_buckets(a.len()));
        let mut value = normalize(spec, &value)?;
        if key == "translationProvider" && value == "native" && !self.native_translation_available {
            value = json!("telegram");
        }
        let changed = self.settings.assign(key, value.clone());
        if key == "shadowBanIds" {
            self.settings.ids_buckets.set(ids_buckets.unwrap());
        }
        if !changed {
            return Ok(vec![]);
        }
        let effects = match key {
            "materialSwitches" => vec![
                Effect::MaterialSwitches(value.as_bool().unwrap()),
                Effect::Repaint,
            ],
            "avatarCorners" => vec![Effect::AvatarCorners(value.as_i64().unwrap() as i32)],
            "singleCornerRadius" => vec![Effect::Repaint],
            "hideNotificationBadge" => vec![Effect::RefreshUnreadBadge],
            "translationProvider" => vec![Effect::ResetTranslationCache],
            "streamerMode" => vec![Effect::StreamerMode(value.as_bool().unwrap())],
            _ => vec![],
        };
        self.emit_effects(&effects);
        self.save()?;
        Ok(effects)
    }
    pub fn set_ghost(&mut self, user: u64, key: &str, value: Value) -> Result<bool, Error> {
        let spec = GHOST_MODE_ACCOUNT_SETTINGS
            .iter()
            .find(|s| s.key == key)
            .ok_or_else(|| Error::Type(key.into()))?;
        let value = normalize(spec, &value)?;
        let changed = self.ghost(user).assign(key, value);
        if changed {
            self.save()?;
        }
        Ok(changed)
    }
    pub fn set_ghost_mode_enabled(
        &mut self,
        user: u64,
        enabled: bool,
        active_session: Option<u64>,
    ) -> Result<Vec<Effect>, Error> {
        let g = self.ghost(user);
        for k in [
            "sendReadMessages",
            "sendReadStories",
            "sendOnlinePackets",
            "sendUploadProgress",
            "sendOfflinePacketAfterOnline",
        ] {
            if g.get(&format!("{k}Locked")) == Some(json!(false)) {
                g.assign(
                    k,
                    json!(if k == "sendOfflinePacketAfterOnline" {
                        enabled
                    } else {
                        !enabled
                    }),
                );
            }
        }
        self.save()?;
        let effects = active_session
            .filter(|_| enabled)
            .map(Effect::MarkAsOnline)
            .into_iter()
            .collect::<Vec<_>>();
        self.emit_effects(&effects);
        Ok(effects)
    }
    pub fn set_message_shot(&self, key: &str, value: Value) -> Result<bool, Error> {
        let spec = MESSAGE_SHOT_SETTINGS
            .iter()
            .find(|s| s.key == key)
            .ok_or_else(|| Error::Type(key.into()))?;
        let changed = self.message_shot.assign(key, normalize(spec, &value)?);
        if changed {
            self.save()?;
        }
        Ok(changed)
    }
    fn clear_cloud_theme_data(&self) {
        for k in [
            "cloudThemeId",
            "cloudThemeAccessHash",
            "cloudThemeDocumentId",
            "cloudThemeAccountId",
        ] {
            self.message_shot.assign(k, json!(0));
        }
        self.message_shot.assign("cloudThemeTitle", json!(""));
    }
    pub fn set_embedded_theme(&self, kind: i32, accent: u32) -> Result<bool, Error> {
        if self.message_shot.get("embeddedThemeType") == Some(json!(kind))
            && self.message_shot.get("embeddedThemeAccentColor") == Some(json!(accent))
            && self.message_shot.is_cloud_theme_empty()
        {
            return Ok(false);
        }
        self.message_shot.assign("embeddedThemeType", json!(kind));
        self.message_shot
            .assign("embeddedThemeAccentColor", json!(accent));
        self.clear_cloud_theme_data();
        self.save()?;
        Ok(true)
    }
    pub fn set_cloud_theme(
        &self,
        account: u64,
        id: u64,
        access: u64,
        document: u64,
        title: &str,
    ) -> Result<bool, Error> {
        let values = [
            ("embeddedThemeType", json!(-1)),
            ("embeddedThemeAccentColor", json!(0)),
            ("cloudThemeAccountId", json!(account)),
            ("cloudThemeId", json!(id)),
            ("cloudThemeAccessHash", json!(access)),
            ("cloudThemeDocumentId", json!(document)),
            ("cloudThemeTitle", json!(title)),
        ];
        if values
            .iter()
            .all(|(k, v)| self.message_shot.get(k).as_ref() == Some(v))
        {
            return Ok(false);
        }
        for (k, v) in values {
            self.message_shot.assign(k, v);
        }
        self.save()?;
        Ok(true)
    }
    pub fn clear_theme(&self) -> Result<bool, Error> {
        self.set_embedded_theme(-1, 0)
    }
    pub fn add_shadow_ban(&mut self, id: i64) -> Result<Vec<Effect>, Error> {
        let mut ids = self
            .settings
            .get("shadowBanIds")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect::<Vec<_>>();
        let mut buckets = self.settings.ids_buckets.get();
        if !crate::json_ext::set_insert(&mut ids, &mut buckets, id) {
            return Ok(vec![]);
        }
        self.settings.ids_buckets.set(buckets);
        self.settings.values["shadowBanIds"].set(json!(ids));
        self.emit_effects(&[Effect::RebuildFilters]);
        self.save()?;
        Ok(vec![Effect::RebuildFilters])
    }
    pub fn remove_shadow_ban(&mut self, id: i64) -> Result<Vec<Effect>, Error> {
        let mut ids = self
            .settings
            .get("shadowBanIds")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
        let len = ids.len();
        ids.retain(|v| *v != id);
        if len == ids.len() {
            return Ok(vec![]);
        }
        self.settings.values["shadowBanIds"].set(ids.into());
        self.emit_effects(&[Effect::RebuildFilters]);
        self.save()?;
        Ok(vec![Effect::RebuildFilters])
    }
    pub fn validate(&self) -> Result<bool, Error> {
        let mut changed = false;
        for (key, min, max) in [
            ("messageBubbleRadius", 0., 16.),
            ("wideMultiplier", 0.5, 4.),
            ("avatarCorners", 0., 23.),
        ] {
            let value = self.settings.get(key).unwrap().as_f64().unwrap();
            if value < min || value > max {
                let spec = AYU_SETTINGS.iter().find(|s| s.key == key).unwrap();
                changed |= self
                    .settings
                    .assign(key, serde_json::from_str(spec.default_json)?);
            }
        }
        if !self.native_translation_available
            && self.settings.get("translationProvider") == Some(json!("native"))
        {
            changed |= self
                .settings
                .assign("translationProvider", json!("telegram"));
        }
        let kind = self
            .message_shot
            .get("embeddedThemeType")
            .unwrap()
            .as_i64()
            .unwrap();
        if !(-1..=3).contains(&kind) {
            changed |= self.message_shot.assign("embeddedThemeType", json!(-1));
            changed |= self
                .message_shot
                .assign("embeddedThemeAccentColor", json!(0));
        }
        if changed {
            self.save()?;
        }
        Ok(changed)
    }
}
fn stoull(s: &str) -> Result<u64, Error> {
    let text = s.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let (negative, text) = if let Some(s) = text.strip_prefix('-') {
        (true, s)
    } else {
        (false, text.strip_prefix('+').unwrap_or(text))
    };
    let digits = text
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    let value = digits
        .parse::<u64>()
        .map_err(|_| Error::Type("invalid account ID".into()))?;
    Ok(if negative {
        value.wrapping_neg()
    } else {
        value
    })
}
fn migrate_ghost(j: &mut Value) -> Result<(), Error> {
    if j.get("ghostModeSettings").is_some() {
        return Ok(());
    }
    let obj = j
        .as_object_mut()
        .ok_or_else(|| Error::Type("expected object".into()))?;
    let mut ghost = Map::new();
    for key in [
        "sendReadMessages",
        "sendReadStories",
        "sendOnlinePackets",
        "sendUploadProgress",
        "sendOfflinePacketAfterOnline",
        "markReadAfterAction",
        "useScheduledMessages",
        "sendWithoutSound",
    ] {
        let default = matches!(
            key,
            "sendReadMessages"
                | "sendReadStories"
                | "sendOnlinePackets"
                | "sendUploadProgress"
                | "markReadAfterAction"
        );
        let value = obj.get(key).cloned().unwrap_or(json!(default));
        if !value.is_boolean() {
            return Err(Error::Type(key.into()));
        }
        ghost.insert(key.into(), value);
    }
    obj.insert("ghostModeSettings".into(), json!({"0":ghost}));
    obj.insert("useGlobalGhostMode".into(), json!(true));
    Ok(())
}
pub fn write_json_file(path: &Path, settings: &AyuSettings) -> Result<(), Error> {
    std::fs::write(path, settings.json_bytes()?)?;
    Ok(())
}

impl AyuSettings {
    pub fn message_shot_settings(&self) -> &MessageShotSettings {
        &self.message_shot
    }
    pub fn is_shadow_banned(&self, id: i64) -> bool {
        self.shadow_ban_ids().contains(&id)
    }
    pub fn shadow_ban_ids(&self) -> Vec<i64> {
        self.settings
            .get("shadowBanIds")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect()
    }
}
include!("ayu_settings_accessors.rs");
