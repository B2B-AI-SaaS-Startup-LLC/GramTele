//! Config fields, server updates, and exact Qt_5_1 tdata format.
use crate::{
    details::qt_stream::{Reader, Writer},
    mtproto_dc_options::{DcOptions, Environment},
};
#[derive(Clone, Debug)]
pub struct ConfigFields {
    /// QString null bits: internal links=1, TXT domain=2, reaction=4, GIF=8, venue=16.
    pub string_nulls: u8,
    pub chat_size_max: i32,
    pub megagroup_size_max: i32,
    pub forwarded_count_max: i32,
    pub online_update_period: i32,
    pub offline_blur_timeout: i32,
    pub offline_idle_timeout: i32,
    pub online_focus_timeout: i32,
    pub online_cloud_timeout: i32,
    pub notify_cloud_delay: i32,
    pub notify_default_delay: i32,
    pub edit_time_limit: i32,
    pub revoke_time_limit: i32,
    pub revoke_private_time_limit: i32,
    pub revoke_private_inbox: bool,
    pub stickers_recent_limit: i32,
    pub internal_links_domain: String,
    pub channels_read_media_period: i32,
    pub call_receive_timeout_ms: i32,
    pub call_ring_timeout_ms: i32,
    pub call_connect_timeout_ms: i32,
    pub call_packet_timeout_ms: i32,
    pub web_file_dc_id: i32,
    pub txt_domain_string: String,
    pub blocked_mode: bool,
    pub caption_length_max: i32,
    pub rating_decay: i32,
    pub reaction_default_emoji: String,
    pub reaction_default_custom: u64,
    pub autologin_token: String,
    pub gif_search_username: String,
    pub venue_search_username: String,
}
impl ConfigFields {
    pub fn new(environment: Environment) -> Self {
        let test = environment == Environment::Test;
        Self {
            string_nulls: 0,
            chat_size_max: 200,
            megagroup_size_max: 10000,
            forwarded_count_max: 100,
            online_update_period: 120000,
            offline_blur_timeout: 5000,
            offline_idle_timeout: 30000,
            online_focus_timeout: 1000,
            online_cloud_timeout: 300000,
            notify_cloud_delay: 30000,
            notify_default_delay: 1500,
            edit_time_limit: 172800,
            revoke_time_limit: 172800,
            revoke_private_time_limit: 172800,
            revoke_private_inbox: false,
            stickers_recent_limit: 30,
            internal_links_domain: "https://t.me/".into(),
            channels_read_media_period: 86400 * 7,
            call_receive_timeout_ms: 20000,
            call_ring_timeout_ms: 90000,
            call_connect_timeout_ms: 30000,
            call_packet_timeout_ms: 10000,
            blocked_mode: false,
            caption_length_max: 1024,
            rating_decay: 2419200,
            reaction_default_custom: 0,
            autologin_token: "".into(),
            web_file_dc_id: if test { 2 } else { 4 },
            txt_domain_string: if test {
                "tapv3.stel.com"
            } else {
                "apv3.stel.com"
            }
            .into(),
            reaction_default_emoji: "👍".into(),
            gif_search_username: if test { "izgifbot" } else { "gif" }.into(),
            venue_search_username: if test { "foursquarebot" } else { "foursquare" }.into(),
        }
    }
}
#[derive(Clone)]
pub struct Config {
    pub dc_options: DcOptions,
    pub fields: ConfigFields,
    pub updates: u64,
}
impl Config {
    pub fn new(environment: Environment) -> Self {
        Self {
            dc_options: DcOptions::new(environment),
            fields: ConfigFields::new(environment),
            updates: 0,
        }
    }
    pub fn environment(&self) -> Environment {
        self.dc_options.environment()
    }
    pub fn is_test_mode(&self) -> bool {
        self.dc_options.is_test_mode()
    }
    pub fn values(&self) -> &ConfigFields {
        &self.fields
    }
    pub fn serialize(&self) -> Vec<u8> {
        let f = &self.fields;
        let mut out = Writer::default();
        out.i32(1);
        out.i32(i32::from(self.is_test_mode()));
        out.bytes(&self.dc_options.serialize());
        out.i32(f.chat_size_max);
        out.i32(f.megagroup_size_max);
        out.i32(f.forwarded_count_max);
        out.i32(f.online_update_period);
        out.i32(f.offline_blur_timeout);
        out.i32(f.offline_idle_timeout);
        out.i32(f.online_focus_timeout);
        out.i32(f.online_cloud_timeout);
        out.i32(f.notify_cloud_delay);
        out.i32(f.notify_default_delay);
        out.i32(0);
        out.i32(f.edit_time_limit);
        out.i32(f.revoke_time_limit);
        out.i32(f.revoke_private_time_limit);
        out.i32(i32::from(f.revoke_private_inbox));
        out.i32(f.stickers_recent_limit);
        out.i32(0);
        out.i32(0);
        out.i32(0);
        write_qstring(&mut out, &f.internal_links_domain, f.string_nulls & 1 != 0);
        out.i32(f.channels_read_media_period);
        out.i32(f.call_receive_timeout_ms);
        out.i32(f.call_ring_timeout_ms);
        out.i32(f.call_connect_timeout_ms);
        out.i32(f.call_packet_timeout_ms);
        out.i32(f.web_file_dc_id);
        write_qstring(&mut out, &f.txt_domain_string, f.string_nulls & 2 != 0);
        out.i32(1);
        out.i32(i32::from(f.blocked_mode));
        out.i32(f.caption_length_max);
        write_qstring(&mut out, &f.reaction_default_emoji, f.string_nulls & 4 != 0);
        out.u64(f.reaction_default_custom);
        out.i32(f.rating_decay);
        write_qstring(&mut out, &f.gif_search_username, f.string_nulls & 8 != 0);
        write_qstring(&mut out, &f.venue_search_username, f.string_nulls & 16 != 0);
        out.0
    }
    pub fn from_serialized(bytes: &[u8]) -> Option<Self> {
        let mut input = Reader::new(bytes);
        if input.i32()? != 1 {
            return None;
        }
        let environment = match input.i32()? {
            0 => Environment::Production,
            1 => Environment::Test,
            _ => return None,
        };
        let mut result = Self::new(environment);
        let options = input.bytes()?;
        let f = &mut result.fields;
        f.chat_size_max = input.i32()?;
        f.megagroup_size_max = input.i32()?;
        f.forwarded_count_max = input.i32()?;
        f.online_update_period = input.i32()?;
        f.offline_blur_timeout = input.i32()?;
        f.offline_idle_timeout = input.i32()?;
        f.online_focus_timeout = input.i32()?;
        f.online_cloud_timeout = input.i32()?;
        f.notify_cloud_delay = input.i32()?;
        f.notify_default_delay = input.i32()?;
        input.i32()?;
        f.edit_time_limit = input.i32()?;
        f.revoke_time_limit = input.i32()?;
        f.revoke_private_time_limit = input.i32()?;
        f.revoke_private_inbox = input.i32()? == 1;
        f.stickers_recent_limit = input.i32()?;
        input.i32()?;
        input.i32()?;
        input.i32()?;
        f.internal_links_domain = read_qstring(&mut input, &mut f.string_nulls, 1)?;
        f.channels_read_media_period = input.i32()?;
        f.call_receive_timeout_ms = input.i32()?;
        f.call_ring_timeout_ms = input.i32()?;
        f.call_connect_timeout_ms = input.i32()?;
        f.call_packet_timeout_ms = input.i32()?;
        f.web_file_dc_id = input.i32()?;
        f.txt_domain_string = read_qstring(&mut input, &mut f.string_nulls, 2)?;
        input.i32()?;
        f.blocked_mode = input.i32()? == 1;
        f.caption_length_max = input.i32()?;
        if !input.at_end() {
            f.reaction_default_emoji = read_qstring(&mut input, &mut f.string_nulls, 4)?;
            f.reaction_default_custom = input.u64()?;
        }
        if !input.at_end() {
            f.rating_decay = input.i32()?;
        }
        if !input.at_end() {
            f.gif_search_username = read_qstring(&mut input, &mut f.string_nulls, 8)?;
            f.venue_search_username = read_qstring(&mut input, &mut f.string_nulls, 16)?;
        }
        if !result.dc_options.construct_from_serialized(&options) {
            return None;
        }
        Some(result)
    }
    pub fn apply(&mut self, data: &tg_scheme::ConfigData) -> bool {
        if data.test_mode != self.is_test_mode() {
            return false;
        }
        self.fields.chat_size_max = data.chat_size_max;
        self.fields.megagroup_size_max = data.megagroup_size_max;
        self.fields.forwarded_count_max = data.forwarded_count_max;
        self.fields.online_update_period = data.online_update_period_ms;
        self.fields.offline_blur_timeout = data.offline_blur_timeout_ms;
        self.fields.offline_idle_timeout = data.offline_idle_timeout_ms;
        self.fields.online_cloud_timeout = data.online_cloud_timeout_ms;
        self.fields.notify_cloud_delay = data.notify_cloud_delay_ms;
        self.fields.notify_default_delay = data.notify_default_delay_ms;
        self.fields.edit_time_limit = data.edit_time_limit;
        self.fields.revoke_time_limit = data.revoke_time_limit;
        self.fields.revoke_private_time_limit = data.revoke_pm_time_limit;
        self.fields.revoke_private_inbox = data.is_revoke_pm_inbox();
        self.fields.stickers_recent_limit = data.stickers_recent_limit;
        self.fields.channels_read_media_period = data.channels_read_media_period;
        self.fields.web_file_dc_id = data.webfile_dc_id;
        self.fields.call_receive_timeout_ms = data.call_receive_timeout_ms;
        self.fields.call_ring_timeout_ms = data.call_ring_timeout_ms;
        self.fields.call_connect_timeout_ms = data.call_connect_timeout_ms;
        self.fields.call_packet_timeout_ms = data.call_packet_timeout_ms;
        self.fields.blocked_mode = data.is_blocked_mode();
        self.fields.caption_length_max = data.caption_length_max;
        self.fields.rating_decay = data.rating_e_decay;
        self.fields.internal_links_domain =
            String::from_utf8_lossy(&data.me_url_prefix).into_owned();
        self.fields.reaction_default_emoji = "👍".into();
        self.fields.reaction_default_custom = 0;
        if let Some(reaction) = &data.reactions_default {
            match reaction {
                tg_scheme::Reaction::ReactionEmpty(_) => {}
                tg_scheme::Reaction::ReactionEmoji(d) => {
                    self.fields.reaction_default_emoji =
                        String::from_utf8_lossy(&d.emoticon).into_owned()
                }
                tg_scheme::Reaction::ReactionCustomEmoji(d) => {
                    self.fields.reaction_default_custom = d.document_id
                }
                tg_scheme::Reaction::ReactionPaid(_) => {
                    self.fields.reaction_default_emoji = "*".into()
                }
            }
        }
        self.fields.autologin_token =
            String::from_utf8_lossy(data.autologin_token.as_deref().unwrap_or_default())
                .into_owned();
        if self.fields.rating_decay <= 0 {
            self.fields.rating_decay = ConfigFields::new(self.environment()).rating_decay;
        }
        self.fields.gif_search_username =
            String::from_utf8_lossy(data.gif_search_username.as_deref().unwrap_or_default())
                .into_owned();
        self.fields.venue_search_username =
            String::from_utf8_lossy(data.venue_search_username.as_deref().unwrap_or_default())
                .into_owned();
        if !data.dc_options.is_empty() {
            self.dc_options.set_from_list(&data.dc_options);
        }
        self.fields.string_nulls = 0;
        if self.fields.internal_links_domain.is_empty() {
            self.fields.string_nulls |= 1;
        }
        if self.fields.txt_domain_string.is_empty() {
            self.fields.string_nulls |= 2;
        }
        if self.fields.reaction_default_emoji.is_empty() {
            self.fields.string_nulls |= 4;
        }
        if self.fields.gif_search_username.is_empty() {
            self.fields.string_nulls |= 8;
        }
        if self.fields.venue_search_username.is_empty() {
            self.fields.string_nulls |= 16;
        }
        self.updates += 1;
        true
    }
    pub fn set_chat_size_max(&mut self, value: i32) {
        self.fields.chat_size_max = value;
    }
    pub fn set_stickers_recent_limit(&mut self, value: i32) {
        self.fields.stickers_recent_limit = value;
    }
    pub fn set_megagroup_size_max(&mut self, value: i32) {
        self.fields.megagroup_size_max = value;
    }
    pub fn set_txt_domain_string(&mut self, value: String) {
        self.fields.string_nulls &= !2;
        self.fields.txt_domain_string = value;
    }
}

fn write_qstring(out: &mut Writer, value: &str, null: bool) {
    if null && value.is_empty() {
        out.i32(-1);
    } else {
        out.string(value);
    }
}
fn read_qstring(input: &mut Reader<'_>, flags: &mut u8, bit: u8) -> Option<String> {
    if input.data.get(input.position..input.position + 4)? == [255; 4] {
        *flags |= bit;
    } else {
        *flags &= !bit;
    }
    input.string()
}
