//! Flag contract only. Full data_chat model is deferred to the next peer package.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatDataFlag {
    Left = 1 << 0,
    Creator = 1 << 2,
    Deactivated = 1 << 3,
    Forbidden = 1 << 4,
    CallActive = 1 << 5,
    CallNotEmpty = 1 << 6,
    CanSetUsername = 1 << 7,
    NoForwards = 1 << 8,
    HasWelcomeMessages = 1 << 9,
    AyuNoForwards = 1 << 31,
}
tg_base::flag_type!(ChatDataFlag: i32);
pub type ChatDataFlags = tg_base::flags::Flags<ChatDataFlag>;
