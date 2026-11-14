//! Flag contract only. Full data_user model is deferred to the next peer package.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserDataFlag {
    Contact = 1 << 0,
    MutualContact = 1 << 1,
    Deleted = 1 << 2,
    Verified = 1 << 3,
    Scam = 1 << 4,
    Fake = 1 << 5,
    BotInlineGeo = 1 << 6,
    Blocked = 1 << 7,
    HasPhoneCalls = 1 << 8,
    PhoneCallsPrivate = 1 << 9,
    Support = 1 << 10,
    CanPinMessages = 1 << 11,
    DiscardMinPhoto = 1 << 12,
    SelfUser = 1 << 13,
    Premium = 1 << 14,
    UnofficialSecurityRisk = 1 << 15,
    VoiceMessagesForbidden = 1 << 16,
    PersonalPhoto = 1 << 17,
    StoriesHidden = 1 << 18,
    HasActiveStories = 1 << 19,
    HasUnreadStories = 1 << 20,
    RequiresPremiumToWrite = 1 << 21,
    HasRequirePremiumToWrite = 1 << 22,
    HasStarsPerMessage = 1 << 23,
    MessageMoneyRestrictionsKnown = 1 << 24,
    ReadDatesPrivate = 1 << 25,
    StoriesCorrespondent = 1 << 26,
    Forum = 1 << 27,
    HasActiveVideoStream = 1 << 28,
    NoForwardsMyEnabled = 1 << 29,
    NoForwardsPeerEnabled = 1 << 30,
}
tg_base::flag_type!(UserDataFlag: u32);
pub type UserDataFlags = tg_base::flags::Flags<UserDataFlag>;
