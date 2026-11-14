//! Peer notification value model; no storage-settings/localstorage code is used.
use tg_scheme as tl;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DefaultNotify {
    User,
    Group,
    Broadcast,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NotifySound {
    pub title: String,
    pub data: String,
    pub id: u64,
    pub none: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MuteValue {
    pub unmute: bool,
    pub forever: bool,
    pub period: i32,
}
impl MuteValue {
    pub fn is_present(self) -> bool {
        self.unmute || self.forever || self.period != 0
    }
    pub fn until(self, now: i32) -> i32 {
        if self.forever {
            i32::MAX
        } else if self.period > 0 {
            (i64::from(now) + i64::from(self.period)).min(i32::MAX.into()) as i32
        } else if self.unmute {
            0
        } else {
            -1
        }
    }
}
fn parse_sound(sound: &tl::NotificationSound) -> NotifySound {
    use tl::NotificationSound::*;
    match sound {
        NotificationSoundDefault(_) => NotifySound::default(),
        NotificationSoundNone(_) => NotifySound {
            none: true,
            ..Default::default()
        },
        NotificationSoundLocal(d) => NotifySound {
            title: String::from_utf8_lossy(&d.title).into(),
            data: String::from_utf8_lossy(&d.data).into(),
            ..Default::default()
        },
        NotificationSoundRingtone(d) => NotifySound {
            id: d.id,
            ..Default::default()
        },
    }
}
fn serialize_sound(sound: &NotifySound) -> tl::NotificationSound {
    if sound.none {
        tl::mtp_notification_sound_none()
    } else if sound.id != 0 {
        tl::mtp_notification_sound_ringtone(sound.id)
    } else if !sound.title.is_empty() {
        tl::mtp_notification_sound_local(
            sound.title.as_bytes().into(),
            sound.data.as_bytes().into(),
        )
    } else {
        tl::mtp_notification_sound_default()
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct NotifyPeerSettingsValue {
    mute: Option<i32>,
    sound: Option<NotifySound>,
    silent: Option<bool>,
    show_previews: Option<bool>,
    stories_muted: Option<bool>,
}
impl NotifyPeerSettingsValue {
    fn parse(data: &tl::PeerNotifySettingsData) -> Self {
        Self {
            mute: data.vmute_until(),
            sound: data.vother_sound().map(parse_sound),
            silent: data.vsilent(),
            show_previews: data.vshow_previews(),
            stories_muted: data.vstories_muted(),
        }
    }
    fn change(&mut self, next: Self) -> bool {
        if *self == next {
            false
        } else {
            *self = next;
            true
        }
    }
    fn serialize(&self) -> tl::InputPeerNotifySettings {
        let mut flags = tl::tl::Flags::<tl::InputPeerNotifySettingsData>::empty();
        for (present, flag) in [
            (
                self.mute.is_some(),
                tl::InputPeerNotifySettingsData::F_MUTE_UNTIL,
            ),
            (
                self.sound.is_some(),
                tl::InputPeerNotifySettingsData::F_SOUND,
            ),
            (
                self.silent.is_some(),
                tl::InputPeerNotifySettingsData::F_SILENT,
            ),
            (
                self.show_previews.is_some(),
                tl::InputPeerNotifySettingsData::F_SHOW_PREVIEWS,
            ),
            (
                self.stories_muted.is_some(),
                tl::InputPeerNotifySettingsData::F_STORIES_MUTED,
            ),
        ] {
            if present {
                flags |= flag;
            }
        }
        tl::mtp_input_peer_notify_settings(
            flags,
            self.show_previews,
            self.silent,
            self.mute,
            self.sound.as_ref().map(serialize_sound),
            self.stories_muted,
            None,
            None,
        )
    }
}
#[derive(Clone, Debug, Default)]
pub struct PeerNotifySettings {
    known: bool,
    value: Option<NotifyPeerSettingsValue>,
}
impl PeerNotifySettings {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn change(&mut self, settings: &tl::PeerNotifySettings) -> bool {
        let tl::PeerNotifySettings::PeerNotifySettings(data) = settings;
        if data.flags.is_empty() {
            if !self.known || self.value.is_some() {
                self.known = true;
                self.value = None;
                return true;
            }
            return false;
        }
        let next = NotifyPeerSettingsValue::parse(data);
        if let Some(value) = &mut self.value {
            value.change(next)
        } else {
            self.known = true;
            self.value = Some(next);
            true
        }
    }
    pub fn change_local(
        &mut self,
        mute: MuteValue,
        silent: Option<bool>,
        sound: Option<NotifySound>,
        stories_muted: Option<bool>,
        now: i32,
    ) -> bool {
        if !mute.is_present() && silent.is_none() && sound.is_none() && stories_muted.is_none() {
            return false;
        }
        if let Some(value) = &mut self.value {
            let next = NotifyPeerSettingsValue {
                mute: if mute.is_present() {
                    Some(mute.until(now))
                } else {
                    value.mute
                },
                silent: silent.or(value.silent),
                sound: sound.or_else(|| value.sound.clone()),
                show_previews: value.show_previews,
                stories_muted: stories_muted.or(value.stories_muted),
            };
            return value.change(next);
        }
        self.known = true;
        self.value = Some(NotifyPeerSettingsValue {
            mute: mute.is_present().then(|| mute.until(now)),
            silent,
            sound,
            stories_muted,
            show_previews: None,
        });
        true
    }
    pub fn reset_to_default(&mut self) -> bool {
        if self.known && self.value.is_none() {
            return false;
        }
        self.known = true;
        self.value = None;
        true
    }
    pub fn settings_unknown(&self) -> bool {
        !self.known
    }
    pub fn mute_until(&self) -> Option<i32> {
        self.value.as_ref().and_then(|v| v.mute)
    }
    pub fn silent_posts(&self) -> Option<bool> {
        self.value.as_ref().and_then(|v| v.silent)
    }
    pub fn sound(&self) -> Option<&NotifySound> {
        self.value.as_ref().and_then(|v| v.sound.as_ref())
    }
    pub fn serialize(&self) -> tl::InputPeerNotifySettings {
        self.value.as_ref().map_or_else(
            || {
                tl::mtp_input_peer_notify_settings(
                    tl::tl::Flags::empty(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            },
            NotifyPeerSettingsValue::serialize,
        )
    }
}
