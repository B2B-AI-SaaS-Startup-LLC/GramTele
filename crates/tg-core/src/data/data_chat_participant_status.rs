//! ChatAdminRight value contract from data_chat_participant_status.h.
//! Remaining participant/rights operations belong to stage4 package 03.
use tg_base::flags::Flags;
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatAdminRight {
    ChangeInfo = 1 << 0,
    PostMessages = 1 << 1,
    EditMessages = 1 << 2,
    DeleteMessages = 1 << 3,
    BanUsers = 1 << 4,
    InviteByLinkOrAdd = 1 << 5,
    PinMessages = 1 << 7,
    AddAdmins = 1 << 9,
    Anonymous = 1 << 10,
    ManageCall = 1 << 11,
    Other = 1 << 12,
    ManageTopics = 1 << 13,
    PostStories = 1 << 14,
    EditStories = 1 << 15,
    DeleteStories = 1 << 16,
    ManageDirect = 1 << 17,
    ManageRanks = 1 << 18,
    ProcessJoinRequests = 1 << 19,
    ManageLinkedPeers = 1 << 20,
    ManageWelcomeMessages = 1 << 21,
}
tg_base::flag_type!(ChatAdminRight: i32);
pub type ChatAdminRights = Flags<ChatAdminRight>;
