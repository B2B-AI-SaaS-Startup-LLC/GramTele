// Typed counterparts of the C++ settings getters, setters and rpl producers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum PeerIdDisplay {
    Hidden = 0,
    TelegramApi = 1,
    BotApi = 2,
}
impl PeerIdDisplay {
    fn json(self) -> Value {
        json!(self as i32)
    }
    fn read(value: Value) -> Self {
        match value.as_i64().unwrap_or(0) {
            1 => Self::TelegramApi,
            2 => Self::BotApi,
            _ => Self::Hidden,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ChannelBottomButton {
    Hidden = 0,
    MuteUnmute = 1,
    DiscussWithFallback = 2,
}
impl ChannelBottomButton {
    fn json(self) -> Value {
        json!(self as i32)
    }
    fn read(value: Value) -> Self {
        match value.as_i64().unwrap_or(0) {
            1 => Self::MuteUnmute,
            2 => Self::DiscussWithFallback,
            _ => Self::Hidden,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ContextMenuVisibility {
    Hidden = 0,
    Visible = 1,
    VisibleWithModifier = 2,
}
impl ContextMenuVisibility {
    fn json(self) -> Value {
        json!(self as i32)
    }
    fn read(value: Value) -> Self {
        match value.as_i64().unwrap_or(0) {
            1 => Self::Visible,
            2 => Self::VisibleWithModifier,
            _ => Self::Hidden,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum TranslationProvider {
    Telegram = 0,
    Google = 1,
    Yandex = 2,
    Native = 3,
}
impl TranslationProvider {
    fn json(self) -> Value {
        json!(match self {
            Self::Telegram => "telegram",
            Self::Google => "google",
            Self::Yandex => "yandex",
            Self::Native => "native",
        })
    }
    fn read(value: Value) -> Self {
        match value.as_str().unwrap_or("") {
            "google" => Self::Google,
            "yandex" => Self::Yandex,
            "native" => Self::Native,
            _ => Self::Telegram,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum SendWithoutSoundOption {
    Never = 0,
    InGhostMode = 1,
    Always = 2,
}
impl SendWithoutSoundOption {
    fn json(self) -> Value {
        json!(self as i32)
    }
    fn read(value: Value) -> Self {
        match value.as_i64().unwrap_or(0) {
            1 => Self::InGhostMode,
            2 => Self::Always,
            _ => Self::Never,
        }
    }
}

trait SettingValue: Clone + 'static {
    fn read_setting(value: Value) -> Self;
    fn setting_json(self) -> Value;
}
macro_rules! primitive_value {
    ($t:ty,$value:ident,$read:expr) => {
        impl SettingValue for $t {
            fn read_setting($value: Value) -> Self {
                $read
            }
            fn setting_json(self) -> Value {
                json!(self)
            }
        }
    };
}
primitive_value!(bool, v, v.as_bool().unwrap());
primitive_value!(i32, v, v.as_i64().unwrap() as i32);
primitive_value!(u32, v, v.as_u64().unwrap() as u32);
primitive_value!(u64, v, v.as_u64().unwrap());
primitive_value!(f64, v, v.as_f64().unwrap());
primitive_value!(String, v, v.as_str().unwrap().to_owned());
macro_rules! enum_value {($($t:ty),*)=>{$(impl SettingValue for $t{fn read_setting(v:Value)->Self{Self::read(v)}fn setting_json(self)->Value{self.json()}})*};}
enum_value!(
    PeerIdDisplay,
    ChannelBottomButton,
    ContextMenuVisibility,
    TranslationProvider,
    SendWithoutSoundOption
);
macro_rules! root_accessors {($(($key:literal,$get:ident,$value:ident,$changes:ident,$set:ident,$t:ty)),*$(,)?)=>{impl AyuSettings{$(
 pub fn $get(&self)->$t{<$t>::read_setting(self.settings.get($key).unwrap())}
 pub fn $value(&self)->Producer<$t>{self.settings.value($key).unwrap().map(<$t>::read_setting)}
 pub fn $changes(&self)->Producer<$t>{self.settings.changes($key).unwrap().map(<$t>::read_setting)}
 pub fn $set(&mut self,v:$t)->Result<Vec<Effect>,Error>{self.set($key,v.setting_json())}
)*}};}
macro_rules! ghost_accessors {($(($key:literal,$get:ident,$value:ident,$changes:ident,$set:ident,$t:ty)),*$(,)?)=>{
 impl GhostModeAccountSettings{$(
 pub fn $get(&self)->$t{<$t>::read_setting(self.0.get($key).unwrap())}
 pub fn $value(&self)->Producer<$t>{self.0.value($key).unwrap().map(<$t>::read_setting)}
 pub fn $changes(&self)->Producer<$t>{self.0.changes($key).unwrap().map(<$t>::read_setting)}
 )*}
 impl AyuSettings{$(pub fn $set(&mut self,user:u64,v:$t)->Result<bool,Error>{self.set_ghost(user,$key,v.setting_json())})*}
};}
macro_rules! shot_accessors {($(($key:literal,$get:ident,$value:ident,$changes:ident,$t:ty)),*$(,)?)=>{impl MessageShotSettings{$(
 pub fn $get(&self)->$t{<$t>::read_setting(self.0.get($key).unwrap())}
 pub fn $value(&self)->Producer<$t>{self.0.value($key).unwrap().map(<$t>::read_setting)}
 pub fn $changes(&self)->Producer<$t>{self.0.changes($key).unwrap().map(<$t>::read_setting)}
)*}};}
macro_rules! shot_setters {($(($key:literal,$set:ident)),*$(,)?)=>{impl AyuSettings{$(pub fn $set(&self,v:bool)->Result<bool,Error>{self.set_message_shot($key,json!(v))})*}};}
root_accessors! {
("useGlobalGhostMode",use_global_ghost_mode,use_global_ghost_mode_value,use_global_ghost_mode_changes,set_use_global_ghost_mode,bool),
("saveDeletedMessages",save_deleted_messages,save_deleted_messages_value,save_deleted_messages_changes,set_save_deleted_messages,bool),
("saveMessagesHistory",save_messages_history,save_messages_history_value,save_messages_history_changes,set_save_messages_history,bool),
("saveForBots",save_for_bots,save_for_bots_value,save_for_bots_changes,set_save_for_bots,bool),
("filtersEnabled",filters_enabled,filters_enabled_value,filters_enabled_changes,set_filters_enabled,bool),
("filtersEnabledInChats",filters_enabled_in_chats,filters_enabled_in_chats_value,filters_enabled_in_chats_changes,set_filters_enabled_in_chats,bool),
("hideFromBlocked",hide_from_blocked,hide_from_blocked_value,hide_from_blocked_changes,set_hide_from_blocked,bool),
("semiTransparentDeletedMessages",semi_transparent_deleted_messages,semi_transparent_deleted_messages_value,semi_transparent_deleted_messages_changes,set_semi_transparent_deleted_messages,bool),
("disableAds",disable_ads,disable_ads_value,disable_ads_changes,set_disable_ads,bool),
("disableStories",disable_stories,disable_stories_value,disable_stories_changes,set_disable_stories,bool),
("disableCustomBackgrounds",disable_custom_backgrounds,disable_custom_backgrounds_value,disable_custom_backgrounds_changes,set_disable_custom_backgrounds,bool),
("hidePremiumStatuses",hide_premium_statuses,hide_premium_statuses_value,hide_premium_statuses_changes,set_hide_premium_statuses,bool),
("showOnlyAddedEmojisAndStickers",show_only_added_emojis_and_stickers,show_only_added_emojis_and_stickers_value,show_only_added_emojis_and_stickers_changes,set_show_only_added_emojis_and_stickers,bool),
("collapseSimilarChannels",collapse_similar_channels,collapse_similar_channels_value,collapse_similar_channels_changes,set_collapse_similar_channels,bool),
("hideSimilarChannels",hide_similar_channels,hide_similar_channels_value,hide_similar_channels_changes,set_hide_similar_channels,bool),
("messageBubbleRadius",message_bubble_radius,message_bubble_radius_value,message_bubble_radius_changes,set_message_bubble_radius,i32),
("disableOpenLinkWarning",disable_open_link_warning,disable_open_link_warning_value,disable_open_link_warning_changes,set_disable_open_link_warning,bool),
("wideMultiplier",wide_multiplier,wide_multiplier_value,wide_multiplier_changes,set_wide_multiplier,f64),
("spoofWebviewAsAndroid",spoof_webview_as_android,spoof_webview_as_android_value,spoof_webview_as_android_changes,set_spoof_webview_as_android,bool),
("increaseWebviewHeight",increase_webview_height,increase_webview_height_value,increase_webview_height_changes,set_increase_webview_height,bool),
("increaseWebviewWidth",increase_webview_width,increase_webview_width_value,increase_webview_width_changes,set_increase_webview_width,bool),
("materialSwitches",material_switches,material_switches_value,material_switches_changes,set_material_switches,bool),
("removeMessageTail",remove_message_tail,remove_message_tail_value,remove_message_tail_changes,set_remove_message_tail,bool),
("disableNotificationsDelay",disable_notifications_delay,disable_notifications_delay_value,disable_notifications_delay_changes,set_disable_notifications_delay,bool),
("localPremium",local_premium,local_premium_value,local_premium_changes,set_local_premium,bool),
("showChannelReactions",show_channel_reactions,show_channel_reactions_value,show_channel_reactions_changes,set_show_channel_reactions,bool),
("showGroupReactions",show_group_reactions,show_group_reactions_value,show_group_reactions_changes,set_show_group_reactions,bool),
("showPrivateChatReactions",show_private_chat_reactions,show_private_chat_reactions_value,show_private_chat_reactions_changes,set_show_private_chat_reactions,bool),
("appIcon",app_icon,app_icon_value,app_icon_changes,set_app_icon,String),
("simpleQuotesAndReplies",simple_quotes_and_replies,simple_quotes_and_replies_value,simple_quotes_and_replies_changes,set_simple_quotes_and_replies,bool),
("hideFastShare",hide_fast_share,hide_fast_share_value,hide_fast_share_changes,set_hide_fast_share,bool),
("replaceBottomInfoWithIcons",replace_bottom_info_with_icons,replace_bottom_info_with_icons_value,replace_bottom_info_with_icons_changes,set_replace_bottom_info_with_icons,bool),
("deletedMark",deleted_mark,deleted_mark_value,deleted_mark_changes,set_deleted_mark,String),
("editedMark",edited_mark,edited_mark_value,edited_mark_changes,set_edited_mark,String),
("unlimitedRecentStickers",unlimited_recent_stickers,unlimited_recent_stickers_value,unlimited_recent_stickers_changes,set_unlimited_recent_stickers,bool),
("showReactionsPanelInContextMenu",show_reactions_panel_in_context_menu,show_reactions_panel_in_context_menu_value,show_reactions_panel_in_context_menu_changes,set_show_reactions_panel_in_context_menu,ContextMenuVisibility),
("showViewsPanelInContextMenu",show_views_panel_in_context_menu,show_views_panel_in_context_menu_value,show_views_panel_in_context_menu_changes,set_show_views_panel_in_context_menu,ContextMenuVisibility),
("showHideMessageInContextMenu",show_hide_message_in_context_menu,show_hide_message_in_context_menu_value,show_hide_message_in_context_menu_changes,set_show_hide_message_in_context_menu,ContextMenuVisibility),
("showUserMessagesInContextMenu",show_user_messages_in_context_menu,show_user_messages_in_context_menu_value,show_user_messages_in_context_menu_changes,set_show_user_messages_in_context_menu,ContextMenuVisibility),
("showMessageDetailsInContextMenu",show_message_details_in_context_menu,show_message_details_in_context_menu_value,show_message_details_in_context_menu_changes,set_show_message_details_in_context_menu,ContextMenuVisibility),
("showRepeatMessageInContextMenu",show_repeat_message_in_context_menu,show_repeat_message_in_context_menu_value,show_repeat_message_in_context_menu_changes,set_show_repeat_message_in_context_menu,ContextMenuVisibility),
("showAddFilterInContextMenu",show_add_filter_in_context_menu,show_add_filter_in_context_menu_value,show_add_filter_in_context_menu_changes,set_show_add_filter_in_context_menu,ContextMenuVisibility),
("showAttachButtonInMessageField",show_attach_button_in_message_field,show_attach_button_in_message_field_value,show_attach_button_in_message_field_changes,set_show_attach_button_in_message_field,bool),
("showCommandsButtonInMessageField",show_commands_button_in_message_field,show_commands_button_in_message_field_value,show_commands_button_in_message_field_changes,set_show_commands_button_in_message_field,bool),
("showEmojiButtonInMessageField",show_emoji_button_in_message_field,show_emoji_button_in_message_field_value,show_emoji_button_in_message_field_changes,set_show_emoji_button_in_message_field,bool),
("showMicrophoneButtonInMessageField",show_microphone_button_in_message_field,show_microphone_button_in_message_field_value,show_microphone_button_in_message_field_changes,set_show_microphone_button_in_message_field,bool),
("showAutoDeleteButtonInMessageField",show_auto_delete_button_in_message_field,show_auto_delete_button_in_message_field_value,show_auto_delete_button_in_message_field_changes,set_show_auto_delete_button_in_message_field,bool),
("showGiftButtonInMessageField",show_gift_button_in_message_field,show_gift_button_in_message_field_value,show_gift_button_in_message_field_changes,set_show_gift_button_in_message_field,bool),
("showAiEditorButtonInMessageField",show_ai_editor_button_in_message_field,show_ai_editor_button_in_message_field_value,show_ai_editor_button_in_message_field_changes,set_show_ai_editor_button_in_message_field,bool),
("showAttachPopup",show_attach_popup,show_attach_popup_value,show_attach_popup_changes,set_show_attach_popup,bool),
("showEmojiPopup",show_emoji_popup,show_emoji_popup_value,show_emoji_popup_changes,set_show_emoji_popup,bool),
("showMyProfileInDrawer",show_my_profile_in_drawer,show_my_profile_in_drawer_value,show_my_profile_in_drawer_changes,set_show_my_profile_in_drawer,bool),
("showBotsInDrawer",show_bots_in_drawer,show_bots_in_drawer_value,show_bots_in_drawer_changes,set_show_bots_in_drawer,bool),
("showNewGroupInDrawer",show_new_group_in_drawer,show_new_group_in_drawer_value,show_new_group_in_drawer_changes,set_show_new_group_in_drawer,bool),
("showNewChannelInDrawer",show_new_channel_in_drawer,show_new_channel_in_drawer_value,show_new_channel_in_drawer_changes,set_show_new_channel_in_drawer,bool),
("showContactsInDrawer",show_contacts_in_drawer,show_contacts_in_drawer_value,show_contacts_in_drawer_changes,set_show_contacts_in_drawer,bool),
("showCallsInDrawer",show_calls_in_drawer,show_calls_in_drawer_value,show_calls_in_drawer_changes,set_show_calls_in_drawer,bool),
("showSavedMessagesInDrawer",show_saved_messages_in_drawer,show_saved_messages_in_drawer_value,show_saved_messages_in_drawer_changes,set_show_saved_messages_in_drawer,bool),
("showLReadToggleInDrawer",show_lread_toggle_in_drawer,show_lread_toggle_in_drawer_value,show_lread_toggle_in_drawer_changes,set_show_lread_toggle_in_drawer,bool),
("showSReadToggleInDrawer",show_sread_toggle_in_drawer,show_sread_toggle_in_drawer_value,show_sread_toggle_in_drawer_changes,set_show_sread_toggle_in_drawer,bool),
("showNightModeToggleInDrawer",show_night_mode_toggle_in_drawer,show_night_mode_toggle_in_drawer_value,show_night_mode_toggle_in_drawer_changes,set_show_night_mode_toggle_in_drawer,bool),
("showGhostToggleInDrawer",show_ghost_toggle_in_drawer,show_ghost_toggle_in_drawer_value,show_ghost_toggle_in_drawer_changes,set_show_ghost_toggle_in_drawer,bool),
("showStreamerToggleInDrawer",show_streamer_toggle_in_drawer,show_streamer_toggle_in_drawer_value,show_streamer_toggle_in_drawer_changes,set_show_streamer_toggle_in_drawer,bool),
("showGhostToggleInTray",show_ghost_toggle_in_tray,show_ghost_toggle_in_tray_value,show_ghost_toggle_in_tray_changes,set_show_ghost_toggle_in_tray,bool),
("showStreamerToggleInTray",show_streamer_toggle_in_tray,show_streamer_toggle_in_tray_value,show_streamer_toggle_in_tray_changes,set_show_streamer_toggle_in_tray,bool),
("monoFont",mono_font,mono_font_value,mono_font_changes,set_mono_font,String),
("hideNotificationCounters",hide_notification_counters,hide_notification_counters_value,hide_notification_counters_changes,set_hide_notification_counters,bool),
("hideNotificationBadge",hide_notification_badge,hide_notification_badge_value,hide_notification_badge_changes,set_hide_notification_badge,bool),
("hideAllChatsFolder",hide_all_chats_folder,hide_all_chats_folder_value,hide_all_chats_folder_changes,set_hide_all_chats_folder,bool),
("channelBottomButton",channel_bottom_button,channel_bottom_button_value,channel_bottom_button_changes,set_channel_bottom_button,ChannelBottomButton),
("quickAdminShortcuts",quick_admin_shortcuts,quick_admin_shortcuts_value,quick_admin_shortcuts_changes,set_quick_admin_shortcuts,bool),
("disableGreetingSticker",disable_greeting_sticker,disable_greeting_sticker_value,disable_greeting_sticker_changes,set_disable_greeting_sticker,bool),
("showPeerId",show_peer_id,show_peer_id_value,show_peer_id_changes,set_show_peer_id,PeerIdDisplay),
("showMessageSeconds",show_message_seconds,show_message_seconds_value,show_message_seconds_changes,set_show_message_seconds,bool),
("showMessageShot",show_message_shot,show_message_shot_value,show_message_shot_changes,set_show_message_shot,bool),
("filterZalgo",filter_zalgo,filter_zalgo_value,filter_zalgo_changes,set_filter_zalgo,bool),
("stickerConfirmation",sticker_confirmation,sticker_confirmation_value,sticker_confirmation_changes,set_sticker_confirmation,bool),
("gifConfirmation",gif_confirmation,gif_confirmation_value,gif_confirmation_changes,set_gif_confirmation,bool),
("voiceConfirmation",voice_confirmation,voice_confirmation_value,voice_confirmation_changes,set_voice_confirmation,bool),
("roundConfirmation",round_confirmation,round_confirmation_value,round_confirmation_changes,set_round_confirmation,bool),
("translationProvider",translation_provider,translation_provider_value,translation_provider_changes,set_translation_provider,TranslationProvider),
("adaptiveCoverColor",adaptive_cover_color,adaptive_cover_color_value,adaptive_cover_color_changes,set_adaptive_cover_color,bool),
("improveLinkPreviews",improve_link_previews,improve_link_previews_value,improve_link_previews_changes,set_improve_link_previews,bool),
("crashReporting",crash_reporting,crash_reporting_value,crash_reporting_changes,set_crash_reporting,bool),
("avatarCorners",avatar_corners,avatar_corners_value,avatar_corners_changes,set_avatar_corners,i32),
("singleCornerRadius",single_corner_radius,single_corner_radius_value,single_corner_radius_changes,set_single_corner_radius,bool),
("streamerMode",streamer_mode,streamer_mode_value,streamer_mode_changes,set_streamer_mode,bool),
}
ghost_accessors! {
("sendReadMessages",send_read_messages,send_read_messages_value,send_read_messages_changes,set_send_read_messages,bool),
("sendReadStories",send_read_stories,send_read_stories_value,send_read_stories_changes,set_send_read_stories,bool),
("sendOnlinePackets",send_online_packets,send_online_packets_value,send_online_packets_changes,set_send_online_packets,bool),
("sendUploadProgress",send_upload_progress,send_upload_progress_value,send_upload_progress_changes,set_send_upload_progress,bool),
("sendOfflinePacketAfterOnline",send_offline_packet_after_online,send_offline_packet_after_online_value,send_offline_packet_after_online_changes,set_send_offline_packet_after_online,bool),
("markReadAfterAction",mark_read_after_action,mark_read_after_action_value,mark_read_after_action_changes,set_mark_read_after_action,bool),
("useScheduledMessages",use_scheduled_messages,use_scheduled_messages_value,use_scheduled_messages_changes,set_use_scheduled_messages,bool),
("sendWithoutSound",send_without_sound,send_without_sound_value,send_without_sound_changes,set_send_without_sound,SendWithoutSoundOption),
("suggestGhostModeBeforeViewingStory",suggest_ghost_mode_before_viewing_story,suggest_ghost_mode_before_viewing_story_value,suggest_ghost_mode_before_viewing_story_changes,set_suggest_ghost_mode_before_viewing_story,bool),
("sendReadMessagesLocked",send_read_messages_locked,send_read_messages_locked_value,send_read_messages_locked_changes,set_send_read_messages_locked,bool),
("sendReadStoriesLocked",send_read_stories_locked,send_read_stories_locked_value,send_read_stories_locked_changes,set_send_read_stories_locked,bool),
("sendOnlinePacketsLocked",send_online_packets_locked,send_online_packets_locked_value,send_online_packets_locked_changes,set_send_online_packets_locked,bool),
("sendUploadProgressLocked",send_upload_progress_locked,send_upload_progress_locked_value,send_upload_progress_locked_changes,set_send_upload_progress_locked,bool),
("sendOfflinePacketAfterOnlineLocked",send_offline_packet_after_online_locked,send_offline_packet_after_online_locked_value,send_offline_packet_after_online_locked_changes,set_send_offline_packet_after_online_locked,bool),
}
shot_accessors! {
("showBackground",show_background,show_background_value,show_background_changes,bool),
("showDate",show_date,show_date_value,show_date_changes,bool),
("showReactions",show_reactions,show_reactions_value,show_reactions_changes,bool),
("showHeaderDecorations",show_header_decorations,show_header_decorations_value,show_header_decorations_changes,bool),
("showColorfulReplies",show_colorful_replies,show_colorful_replies_value,show_colorful_replies_changes,bool),
("revealSpoilers",reveal_spoilers,reveal_spoilers_value,reveal_spoilers_changes,bool),
("embeddedThemeType",embedded_theme_type,embedded_theme_type_value,embedded_theme_type_changes,i32),
("embeddedThemeAccentColor",embedded_theme_accent_color,embedded_theme_accent_color_value,embedded_theme_accent_color_changes,u32),
("cloudThemeId",cloud_theme_id,cloud_theme_id_value,cloud_theme_id_changes,u64),
("cloudThemeAccessHash",cloud_theme_access_hash,cloud_theme_access_hash_value,cloud_theme_access_hash_changes,u64),
("cloudThemeDocumentId",cloud_theme_document_id,cloud_theme_document_id_value,cloud_theme_document_id_changes,u64),
("cloudThemeTitle",cloud_theme_title,cloud_theme_title_value,cloud_theme_title_changes,String),
("cloudThemeAccountId",cloud_theme_account_id,cloud_theme_account_id_value,cloud_theme_account_id_changes,u64),
}
shot_setters! {("showBackground",set_show_background),("showDate",set_show_date),("showReactions",set_show_reactions),("showHeaderDecorations",set_show_header_decorations),("showColorfulReplies",set_show_colorful_replies),("revealSpoilers",set_reveal_spoilers)}
