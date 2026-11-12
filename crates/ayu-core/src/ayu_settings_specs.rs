// Metadata translated from ayu_settings.h and from_json assignment order.
pub const AYU_SETTINGS: &[Spec] = &[
    Spec {
        key: "useGlobalGhostMode",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "saveDeletedMessages",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "saveMessagesHistory",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "saveForBots",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "shadowBanIds",
        kind: Kind::Ids,
        default_json: "[]",
    },
    Spec {
        key: "filtersEnabled",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "filtersEnabledInChats",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "hideFromBlocked",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "semiTransparentDeletedMessages",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "disableAds",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "disableStories",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "disableCustomBackgrounds",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "hidePremiumStatuses",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "showOnlyAddedEmojisAndStickers",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "collapseSimilarChannels",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "hideSimilarChannels",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "messageBubbleRadius",
        kind: Kind::Int,
        default_json: "16",
    },
    Spec {
        key: "disableOpenLinkWarning",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "wideMultiplier",
        kind: Kind::Float,
        default_json: "1.0",
    },
    Spec {
        key: "spoofWebviewAsAndroid",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "increaseWebviewHeight",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "increaseWebviewWidth",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "materialSwitches",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "removeMessageTail",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "disableNotificationsDelay",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "localPremium",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "showChannelReactions",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showGroupReactions",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showPrivateChatReactions",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "appIcon",
        kind: Kind::Text,
        default_json: "\"default\"",
    },
    Spec {
        key: "simpleQuotesAndReplies",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "hideFastShare",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "replaceBottomInfoWithIcons",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "deletedMark",
        kind: Kind::Text,
        default_json: "\"🧹\"",
    },
    Spec {
        key: "editedMark",
        kind: Kind::Text,
        default_json: "\"edited\"",
    },
    Spec {
        key: "unlimitedRecentStickers",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "showReactionsPanelInContextMenu",
        kind: Kind::Enum,
        default_json: "1",
    },
    Spec {
        key: "showViewsPanelInContextMenu",
        kind: Kind::Enum,
        default_json: "1",
    },
    Spec {
        key: "showHideMessageInContextMenu",
        kind: Kind::Enum,
        default_json: "0",
    },
    Spec {
        key: "showUserMessagesInContextMenu",
        kind: Kind::Enum,
        default_json: "2",
    },
    Spec {
        key: "showMessageDetailsInContextMenu",
        kind: Kind::Enum,
        default_json: "2",
    },
    Spec {
        key: "showRepeatMessageInContextMenu",
        kind: Kind::Enum,
        default_json: "0",
    },
    Spec {
        key: "showAddFilterInContextMenu",
        kind: Kind::Enum,
        default_json: "1",
    },
    Spec {
        key: "showAttachButtonInMessageField",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showCommandsButtonInMessageField",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showEmojiButtonInMessageField",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showMicrophoneButtonInMessageField",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showAutoDeleteButtonInMessageField",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showGiftButtonInMessageField",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showAiEditorButtonInMessageField",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showAttachPopup",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showEmojiPopup",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showMyProfileInDrawer",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showBotsInDrawer",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showNewGroupInDrawer",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showNewChannelInDrawer",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showContactsInDrawer",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showCallsInDrawer",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showSavedMessagesInDrawer",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showLReadToggleInDrawer",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "showSReadToggleInDrawer",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showNightModeToggleInDrawer",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showGhostToggleInDrawer",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showStreamerToggleInDrawer",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "showGhostToggleInTray",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showStreamerToggleInTray",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "monoFont",
        kind: Kind::Text,
        default_json: "\"\"",
    },
    Spec {
        key: "hideNotificationCounters",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "hideNotificationBadge",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "hideAllChatsFolder",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "channelBottomButton",
        kind: Kind::Enum,
        default_json: "2",
    },
    Spec {
        key: "quickAdminShortcuts",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "disableGreetingSticker",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "showPeerId",
        kind: Kind::Enum,
        default_json: "2",
    },
    Spec {
        key: "showMessageSeconds",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "showMessageShot",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "filterZalgo",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "stickerConfirmation",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "gifConfirmation",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "voiceConfirmation",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "roundConfirmation",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "translationProvider",
        kind: Kind::Translation,
        default_json: "\"telegram\"",
    },
    Spec {
        key: "adaptiveCoverColor",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "improveLinkPreviews",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "crashReporting",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "avatarCorners",
        kind: Kind::Int,
        default_json: "23",
    },
    Spec {
        key: "singleCornerRadius",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "streamerMode",
        kind: Kind::Bool,
        default_json: "false",
    },
];
pub const GHOST_MODE_ACCOUNT_SETTINGS: &[Spec] = &[
    Spec {
        key: "sendReadMessages",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "sendReadStories",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "sendOnlinePackets",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "sendUploadProgress",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "sendOfflinePacketAfterOnline",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "markReadAfterAction",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "useScheduledMessages",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "sendWithoutSound",
        kind: Kind::Enum,
        default_json: "0",
    },
    Spec {
        key: "suggestGhostModeBeforeViewingStory",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "sendReadMessagesLocked",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "sendReadStoriesLocked",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "sendOnlinePacketsLocked",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "sendUploadProgressLocked",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "sendOfflinePacketAfterOnlineLocked",
        kind: Kind::Bool,
        default_json: "false",
    },
];
pub const MESSAGE_SHOT_SETTINGS: &[Spec] = &[
    Spec {
        key: "showBackground",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showDate",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "showReactions",
        kind: Kind::Bool,
        default_json: "false",
    },
    Spec {
        key: "showHeaderDecorations",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "showColorfulReplies",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "revealSpoilers",
        kind: Kind::Bool,
        default_json: "true",
    },
    Spec {
        key: "embeddedThemeType",
        kind: Kind::Int,
        default_json: "-1",
    },
    Spec {
        key: "embeddedThemeAccentColor",
        kind: Kind::U32,
        default_json: "0",
    },
    Spec {
        key: "cloudThemeId",
        kind: Kind::U64,
        default_json: "0",
    },
    Spec {
        key: "cloudThemeAccessHash",
        kind: Kind::U64,
        default_json: "0",
    },
    Spec {
        key: "cloudThemeDocumentId",
        kind: Kind::U64,
        default_json: "0",
    },
    Spec {
        key: "cloudThemeTitle",
        kind: Kind::Text,
        default_json: "\"\"",
    },
    Spec {
        key: "cloudThemeAccountId",
        kind: Kind::U64,
        default_json: "0",
    },
];
