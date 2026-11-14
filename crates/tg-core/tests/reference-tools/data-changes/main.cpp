// Test-only access to the real private manager. No replacement model bodies.
#define private public
#include "data/data_changes.h"
#undef private
#include "data/data_changes.cpp"

void shifts() {
    Data::Changes::Manager<PeerData, Data::PeerUpdate> manager;
    // Opaque identities: never dereferenced or passed to model methods.
    alignas(64) unsigned char identity[64] = {};
    auto peer = gsl::not_null(reinterpret_cast<PeerData*>(identity));
    rpl::lifetime lifetime;
    for (int i = 0; i != 58; ++i) {
        manager.realtimeUpdates(static_cast<Data::PeerUpdate::Flag>(1ULL << i))
        | rpl::on_next([=](const Data::PeerUpdate &update) {
            std::cout << "r," << i << ',' << uint64(update.flags.value()) << '\n';
        }, lifetime);
    }
    manager.updates(Data::PeerUpdate::Flags::from_raw(~uint64(0)))
    | rpl::on_next([](const Data::PeerUpdate &update) {
        std::cout << "p," << uint64(update.flags.value()) << '\n';
    }, lifetime);
    for (int bit = 0; bit != 58; ++bit) {
        std::cout << "bit," << bit << '\n';
        manager.updated(peer, static_cast<Data::PeerUpdate::Flag>(1ULL << bit));
        manager.sendNotifications();
    }
    for (uint64 bits : {uint64(1) | (uint64(1)<<32) | (uint64(1)<<57), (uint64(1)<<25) | (uint64(1)<<57), ~uint64(0)}) {
        std::cout << "mixed," << bits << '\n';
        manager.updated(peer, Data::PeerUpdate::Flags::from_raw(bits));
        manager.sendNotifications();
    }

}

template<typename D, typename U>
void scenario(const char *name) {
    std::cout << "manager," << name << '\n';
    Data::Changes::Manager<D,U> manager;
    alignas(64) unsigned char identities[128] = {};
    auto a = gsl::not_null(reinterpret_cast<D*>(identities));
    auto b = gsl::not_null(reinterpret_cast<D*>(identities + 64));
    auto key = [=](auto data) { return data == a ? 1 : 2; };
    auto flags = [](uint64 bits) { return U::Flags::from_raw(bits); };
    auto record = [=](const char *what, int stream, const U &u) {
        auto &[data, bits] = u;
        std::cout << what << ',' << stream << ',' << key(data) << ',' << uint64(bits.value()) << '\n';
    };
    rpl::lifetime lifetime;
    for (int i=0; i != Data::details::CountBit<typename U::Flag>() + 1; ++i) {
        manager.realtimeUpdates(static_cast<typename U::Flag>(1ULL << i))
        | rpl::on_next([=](const U &u) { record("r",i,u); }, lifetime);
    }
    manager.updates(flags(~uint64(0))) | rpl::on_next([=](const U &u) { record("p",0,u); }, lifetime);
    manager.updates(a,flags(2)) | rpl::on_next([=](const U &u) { record("f",0,u); }, lifetime);
    manager.flagsValue(a,flags(4)) | rpl::on_next([=](const U &u) { record("v",0,u); }, lifetime);
    manager.updated(b,flags(2));
    manager.updated(a,flags(1));
    manager.updated(a,flags(2));
    manager.updated(a,flags(2));
    manager.sendNotifications();
    manager.updated(a,flags(4));
    manager.updated(a,flags(8),true);
    manager.updated(a,flags(16));
    manager.drop(a);
    manager.sendNotifications();
    manager.updated(a,flags(0));
    manager.sendNotifications(); // all filters reject zero
    // A realtime callback flushes older actions before updated queues itself.
    manager.realtimeUpdates(static_cast<typename U::Flag>(2))
    | rpl::on_next([&](const U &u) {
        auto &[data,bits] = u;
        if (uint64(bits.value()) == 2) {
            manager.sendNotifications();
            manager.updated(b,flags(4));
        }
    }, lifetime);
    manager.updated(a,flags(1));
    manager.updated(a,flags(2));
    manager.sendNotifications();
    // A postponed callback queues a new update and recursively flushes it.
    bool nested = false;
    manager.updates(flags(8)) | rpl::on_next([&](const U &) {
        if (!nested) {
            nested = true;
            manager.updated(b,flags(16));
            manager.sendNotifications();
        }
    }, lifetime);
    manager.updated(a,flags(8));
    manager.updated(b,flags(1));
    manager.sendNotifications();
    manager.sendNotifications();
}
void order() {
    std::cout << "changes,order\n";
    alignas(64) unsigned char identities[512] = {};
    // Session is only stored, never dereferenced: scheduling is not exercised.
    Data::Changes changes(gsl::not_null(reinterpret_cast<Main::Session*>(identities)));
    rpl::lifetime lifetime;
    changes._peerChanges.updates(Data::PeerUpdate::Flags::from_raw(1)) | rpl::on_next([](const auto &) { std::cout << "peer\n"; }, lifetime);
    changes._peerChanges.updated(gsl::not_null(reinterpret_cast<PeerData*>(identities+64)),Data::PeerUpdate::Flags::from_raw(1));
    changes._historyChanges.updates(Data::HistoryUpdate::Flags::from_raw(1)) | rpl::on_next([](const auto &) { std::cout << "history\n"; }, lifetime);
    changes._historyChanges.updated(gsl::not_null(reinterpret_cast<History*>(identities+128)),Data::HistoryUpdate::Flags::from_raw(1));
    changes._messageChanges.updates(Data::MessageUpdate::Flags::from_raw(1)) | rpl::on_next([](const auto &) { std::cout << "message\n"; }, lifetime);
    changes._messageChanges.updated(gsl::not_null(reinterpret_cast<HistoryItem*>(identities+192)),Data::MessageUpdate::Flags::from_raw(1));
    changes._entryChanges.updates(Data::EntryUpdate::Flags::from_raw(1)) | rpl::on_next([](const auto &) { std::cout << "entry\n"; }, lifetime);
    changes._entryChanges.updated(gsl::not_null(reinterpret_cast<Dialogs::Entry*>(identities+256)),Data::EntryUpdate::Flags::from_raw(1));
    changes._topicChanges.updates(Data::TopicUpdate::Flags::from_raw(1)) | rpl::on_next([](const auto &) { std::cout << "topic\n"; }, lifetime);
    changes._topicChanges.updated(gsl::not_null(reinterpret_cast<Data::ForumTopic*>(identities+320)),Data::TopicUpdate::Flags::from_raw(1));
    changes._sublistChanges.updates(Data::SublistUpdate::Flags::from_raw(1)) | rpl::on_next([](const auto &) { std::cout << "sublist\n"; }, lifetime);
    changes._sublistChanges.updated(gsl::not_null(reinterpret_cast<Data::SavedSublist*>(identities+384)),Data::SublistUpdate::Flags::from_raw(1));
    changes._storyChanges.updates(Data::StoryUpdate::Flags::from_raw(1)) | rpl::on_next([](const auto &) { std::cout << "story\n"; }, lifetime);
    changes._storyChanges.updated(gsl::not_null(reinterpret_cast<Data::Story*>(identities+448)),Data::StoryUpdate::Flags::from_raw(1));
    changes._notify = true;
    changes.sendNotifications();
    changes.sendNotifications();
}
void declarations() {
    std::cout << "Peer,None," << uint64(Data::PeerUpdate::Flag::None) << "\n";
    std::cout << "Peer,Name," << uint64(Data::PeerUpdate::Flag::Name) << "\n";
    std::cout << "Peer,Username," << uint64(Data::PeerUpdate::Flag::Username) << "\n";
    std::cout << "Peer,Photo," << uint64(Data::PeerUpdate::Flag::Photo) << "\n";
    std::cout << "Peer,About," << uint64(Data::PeerUpdate::Flag::About) << "\n";
    std::cout << "Peer,Notifications," << uint64(Data::PeerUpdate::Flag::Notifications) << "\n";
    std::cout << "Peer,Migration," << uint64(Data::PeerUpdate::Flag::Migration) << "\n";
    std::cout << "Peer,UnavailableReason," << uint64(Data::PeerUpdate::Flag::UnavailableReason) << "\n";
    std::cout << "Peer,ChatThemeToken," << uint64(Data::PeerUpdate::Flag::ChatThemeToken) << "\n";
    std::cout << "Peer,ChatWallPaper," << uint64(Data::PeerUpdate::Flag::ChatWallPaper) << "\n";
    std::cout << "Peer,IsBlocked," << uint64(Data::PeerUpdate::Flag::IsBlocked) << "\n";
    std::cout << "Peer,MessagesTTL," << uint64(Data::PeerUpdate::Flag::MessagesTTL) << "\n";
    std::cout << "Peer,FullInfo," << uint64(Data::PeerUpdate::Flag::FullInfo) << "\n";
    std::cout << "Peer,Usernames," << uint64(Data::PeerUpdate::Flag::Usernames) << "\n";
    std::cout << "Peer,TranslationDisabled," << uint64(Data::PeerUpdate::Flag::TranslationDisabled) << "\n";
    std::cout << "Peer,Color," << uint64(Data::PeerUpdate::Flag::Color) << "\n";
    std::cout << "Peer,ColorProfile," << uint64(Data::PeerUpdate::Flag::ColorProfile) << "\n";
    std::cout << "Peer,BackgroundEmoji," << uint64(Data::PeerUpdate::Flag::BackgroundEmoji) << "\n";
    std::cout << "Peer,StoriesState," << uint64(Data::PeerUpdate::Flag::StoriesState) << "\n";
    std::cout << "Peer,VerifyInfo," << uint64(Data::PeerUpdate::Flag::VerifyInfo) << "\n";
    std::cout << "Peer,StarsPerMessage," << uint64(Data::PeerUpdate::Flag::StarsPerMessage) << "\n";
    std::cout << "Peer,CanShareContact," << uint64(Data::PeerUpdate::Flag::CanShareContact) << "\n";
    std::cout << "Peer,IsContact," << uint64(Data::PeerUpdate::Flag::IsContact) << "\n";
    std::cout << "Peer,PhoneNumber," << uint64(Data::PeerUpdate::Flag::PhoneNumber) << "\n";
    std::cout << "Peer,OnlineStatus," << uint64(Data::PeerUpdate::Flag::OnlineStatus) << "\n";
    std::cout << "Peer,BotCommands," << uint64(Data::PeerUpdate::Flag::BotCommands) << "\n";
    std::cout << "Peer,BotCanBeInvited," << uint64(Data::PeerUpdate::Flag::BotCanBeInvited) << "\n";
    std::cout << "Peer,BotStartToken," << uint64(Data::PeerUpdate::Flag::BotStartToken) << "\n";
    std::cout << "Peer,CommonChats," << uint64(Data::PeerUpdate::Flag::CommonChats) << "\n";
    std::cout << "Peer,PeerGifts," << uint64(Data::PeerUpdate::Flag::PeerGifts) << "\n";
    std::cout << "Peer,HasCalls," << uint64(Data::PeerUpdate::Flag::HasCalls) << "\n";
    std::cout << "Peer,SupportInfo," << uint64(Data::PeerUpdate::Flag::SupportInfo) << "\n";
    std::cout << "Peer,IsBot," << uint64(Data::PeerUpdate::Flag::IsBot) << "\n";
    std::cout << "Peer,EmojiStatus," << uint64(Data::PeerUpdate::Flag::EmojiStatus) << "\n";
    std::cout << "Peer,BusinessDetails," << uint64(Data::PeerUpdate::Flag::BusinessDetails) << "\n";
    std::cout << "Peer,Birthday," << uint64(Data::PeerUpdate::Flag::Birthday) << "\n";
    std::cout << "Peer,PersonalChannel," << uint64(Data::PeerUpdate::Flag::PersonalChannel) << "\n";
    std::cout << "Peer,StarRefProgram," << uint64(Data::PeerUpdate::Flag::StarRefProgram) << "\n";
    std::cout << "Peer,PaysPerMessage," << uint64(Data::PeerUpdate::Flag::PaysPerMessage) << "\n";
    std::cout << "Peer,GiftSettings," << uint64(Data::PeerUpdate::Flag::GiftSettings) << "\n";
    std::cout << "Peer,StarsRating," << uint64(Data::PeerUpdate::Flag::StarsRating) << "\n";
    std::cout << "Peer,ContactNote," << uint64(Data::PeerUpdate::Flag::ContactNote) << "\n";
    std::cout << "Peer,InviteLinks," << uint64(Data::PeerUpdate::Flag::InviteLinks) << "\n";
    std::cout << "Peer,Members," << uint64(Data::PeerUpdate::Flag::Members) << "\n";
    std::cout << "Peer,Admins," << uint64(Data::PeerUpdate::Flag::Admins) << "\n";
    std::cout << "Peer,BannedUsers," << uint64(Data::PeerUpdate::Flag::BannedUsers) << "\n";
    std::cout << "Peer,Rights," << uint64(Data::PeerUpdate::Flag::Rights) << "\n";
    std::cout << "Peer,PendingRequests," << uint64(Data::PeerUpdate::Flag::PendingRequests) << "\n";
    std::cout << "Peer,Reactions," << uint64(Data::PeerUpdate::Flag::Reactions) << "\n";
    std::cout << "Peer,ChannelAmIn," << uint64(Data::PeerUpdate::Flag::ChannelAmIn) << "\n";
    std::cout << "Peer,StickersSet," << uint64(Data::PeerUpdate::Flag::StickersSet) << "\n";
    std::cout << "Peer,EmojiSet," << uint64(Data::PeerUpdate::Flag::EmojiSet) << "\n";
    std::cout << "Peer,DiscussionLink," << uint64(Data::PeerUpdate::Flag::DiscussionLink) << "\n";
    std::cout << "Peer,MonoforumLink," << uint64(Data::PeerUpdate::Flag::MonoforumLink) << "\n";
    std::cout << "Peer,ChannelLocation," << uint64(Data::PeerUpdate::Flag::ChannelLocation) << "\n";
    std::cout << "Peer,Slowmode," << uint64(Data::PeerUpdate::Flag::Slowmode) << "\n";
    std::cout << "Peer,GroupCall," << uint64(Data::PeerUpdate::Flag::GroupCall) << "\n";
    std::cout << "Peer,ManagedBot," << uint64(Data::PeerUpdate::Flag::ManagedBot) << "\n";
    std::cout << "Peer,MainProfileTab," << uint64(Data::PeerUpdate::Flag::MainProfileTab) << "\n";
    std::cout << "Peer,LastUsedBit," << uint64(Data::PeerUpdate::Flag::LastUsedBit) << "\n";
    std::cout << "History,None," << uint64(Data::HistoryUpdate::Flag::None) << "\n";
    std::cout << "History,IsPinned," << uint64(Data::HistoryUpdate::Flag::IsPinned) << "\n";
    std::cout << "History,UnreadView," << uint64(Data::HistoryUpdate::Flag::UnreadView) << "\n";
    std::cout << "History,TopPromoted," << uint64(Data::HistoryUpdate::Flag::TopPromoted) << "\n";
    std::cout << "History,Folder," << uint64(Data::HistoryUpdate::Flag::Folder) << "\n";
    std::cout << "History,UnreadMentions," << uint64(Data::HistoryUpdate::Flag::UnreadMentions) << "\n";
    std::cout << "History,UnreadReactions," << uint64(Data::HistoryUpdate::Flag::UnreadReactions) << "\n";
    std::cout << "History,ClientSideMessages," << uint64(Data::HistoryUpdate::Flag::ClientSideMessages) << "\n";
    std::cout << "History,ChatOccupied," << uint64(Data::HistoryUpdate::Flag::ChatOccupied) << "\n";
    std::cout << "History,MessageSent," << uint64(Data::HistoryUpdate::Flag::MessageSent) << "\n";
    std::cout << "History,ScheduledSent," << uint64(Data::HistoryUpdate::Flag::ScheduledSent) << "\n";
    std::cout << "History,OutboxRead," << uint64(Data::HistoryUpdate::Flag::OutboxRead) << "\n";
    std::cout << "History,BotKeyboard," << uint64(Data::HistoryUpdate::Flag::BotKeyboard) << "\n";
    std::cout << "History,CloudDraft," << uint64(Data::HistoryUpdate::Flag::CloudDraft) << "\n";
    std::cout << "History,TranslateFrom," << uint64(Data::HistoryUpdate::Flag::TranslateFrom) << "\n";
    std::cout << "History,TranslatedTo," << uint64(Data::HistoryUpdate::Flag::TranslatedTo) << "\n";
    std::cout << "History,UnreadPollVotes," << uint64(Data::HistoryUpdate::Flag::UnreadPollVotes) << "\n";
    std::cout << "History,StreamedDrafts," << uint64(Data::HistoryUpdate::Flag::StreamedDrafts) << "\n";
    std::cout << "History,LastUsedBit," << uint64(Data::HistoryUpdate::Flag::LastUsedBit) << "\n";
    std::cout << "Topic,None," << uint64(Data::TopicUpdate::Flag::None) << "\n";
    std::cout << "Topic,UnreadView," << uint64(Data::TopicUpdate::Flag::UnreadView) << "\n";
    std::cout << "Topic,UnreadMentions," << uint64(Data::TopicUpdate::Flag::UnreadMentions) << "\n";
    std::cout << "Topic,UnreadReactions," << uint64(Data::TopicUpdate::Flag::UnreadReactions) << "\n";
    std::cout << "Topic,Notifications," << uint64(Data::TopicUpdate::Flag::Notifications) << "\n";
    std::cout << "Topic,Title," << uint64(Data::TopicUpdate::Flag::Title) << "\n";
    std::cout << "Topic,IconId," << uint64(Data::TopicUpdate::Flag::IconId) << "\n";
    std::cout << "Topic,ColorId," << uint64(Data::TopicUpdate::Flag::ColorId) << "\n";
    std::cout << "Topic,CloudDraft," << uint64(Data::TopicUpdate::Flag::CloudDraft) << "\n";
    std::cout << "Topic,Closed," << uint64(Data::TopicUpdate::Flag::Closed) << "\n";
    std::cout << "Topic,Creator," << uint64(Data::TopicUpdate::Flag::Creator) << "\n";
    std::cout << "Topic,Destroyed," << uint64(Data::TopicUpdate::Flag::Destroyed) << "\n";
    std::cout << "Topic,UnreadPollVotes," << uint64(Data::TopicUpdate::Flag::UnreadPollVotes) << "\n";
    std::cout << "Topic,LastUsedBit," << uint64(Data::TopicUpdate::Flag::LastUsedBit) << "\n";
    std::cout << "Sublist,None," << uint64(Data::SublistUpdate::Flag::None) << "\n";
    std::cout << "Sublist,UnreadView," << uint64(Data::SublistUpdate::Flag::UnreadView) << "\n";
    std::cout << "Sublist,UnreadReactions," << uint64(Data::SublistUpdate::Flag::UnreadReactions) << "\n";
    std::cout << "Sublist,CloudDraft," << uint64(Data::SublistUpdate::Flag::CloudDraft) << "\n";
    std::cout << "Sublist,Destroyed," << uint64(Data::SublistUpdate::Flag::Destroyed) << "\n";
    std::cout << "Sublist,UnreadPollVotes," << uint64(Data::SublistUpdate::Flag::UnreadPollVotes) << "\n";
    std::cout << "Sublist,LastUsedBit," << uint64(Data::SublistUpdate::Flag::LastUsedBit) << "\n";
    std::cout << "Message,None," << uint64(Data::MessageUpdate::Flag::None) << "\n";
    std::cout << "Message,Edited," << uint64(Data::MessageUpdate::Flag::Edited) << "\n";
    std::cout << "Message,Destroyed," << uint64(Data::MessageUpdate::Flag::Destroyed) << "\n";
    std::cout << "Message,DialogRowRepaint," << uint64(Data::MessageUpdate::Flag::DialogRowRepaint) << "\n";
    std::cout << "Message,DialogRowRefresh," << uint64(Data::MessageUpdate::Flag::DialogRowRefresh) << "\n";
    std::cout << "Message,NewAdded," << uint64(Data::MessageUpdate::Flag::NewAdded) << "\n";
    std::cout << "Message,ReplyMarkup," << uint64(Data::MessageUpdate::Flag::ReplyMarkup) << "\n";
    std::cout << "Message,BotCallbackSent," << uint64(Data::MessageUpdate::Flag::BotCallbackSent) << "\n";
    std::cout << "Message,NewMaybeAdded," << uint64(Data::MessageUpdate::Flag::NewMaybeAdded) << "\n";
    std::cout << "Message,ReplyToTopAdded," << uint64(Data::MessageUpdate::Flag::ReplyToTopAdded) << "\n";
    std::cout << "Message,NewUnreadReaction," << uint64(Data::MessageUpdate::Flag::NewUnreadReaction) << "\n";
    std::cout << "Message,LastUsedBit," << uint64(Data::MessageUpdate::Flag::LastUsedBit) << "\n";
    std::cout << "Entry,None," << uint64(Data::EntryUpdate::Flag::None) << "\n";
    std::cout << "Entry,Repaint," << uint64(Data::EntryUpdate::Flag::Repaint) << "\n";
    std::cout << "Entry,HasPinnedMessages," << uint64(Data::EntryUpdate::Flag::HasPinnedMessages) << "\n";
    std::cout << "Entry,ForwardDraft," << uint64(Data::EntryUpdate::Flag::ForwardDraft) << "\n";
    std::cout << "Entry,LocalDraftSet," << uint64(Data::EntryUpdate::Flag::LocalDraftSet) << "\n";
    std::cout << "Entry,Height," << uint64(Data::EntryUpdate::Flag::Height) << "\n";
    std::cout << "Entry,Destroyed," << uint64(Data::EntryUpdate::Flag::Destroyed) << "\n";
    std::cout << "Entry,LastUsedBit," << uint64(Data::EntryUpdate::Flag::LastUsedBit) << "\n";
    std::cout << "Story,None," << uint64(Data::StoryUpdate::Flag::None) << "\n";
    std::cout << "Story,Edited," << uint64(Data::StoryUpdate::Flag::Edited) << "\n";
    std::cout << "Story,Destroyed," << uint64(Data::StoryUpdate::Flag::Destroyed) << "\n";
    std::cout << "Story,NewAdded," << uint64(Data::StoryUpdate::Flag::NewAdded) << "\n";
    std::cout << "Story,ViewsChanged," << uint64(Data::StoryUpdate::Flag::ViewsChanged) << "\n";
    std::cout << "Story,MarkRead," << uint64(Data::StoryUpdate::Flag::MarkRead) << "\n";
    std::cout << "Story,Reaction," << uint64(Data::StoryUpdate::Flag::Reaction) << "\n";
    std::cout << "Story,LastUsedBit," << uint64(Data::StoryUpdate::Flag::LastUsedBit) << "\n";
    std::cout << "ChatAdminRight,ChangeInfo," << int(ChatAdminRight::ChangeInfo) << "\n";
    std::cout << "ChatAdminRight,PostMessages," << int(ChatAdminRight::PostMessages) << "\n";
    std::cout << "ChatAdminRight,EditMessages," << int(ChatAdminRight::EditMessages) << "\n";
    std::cout << "ChatAdminRight,DeleteMessages," << int(ChatAdminRight::DeleteMessages) << "\n";
    std::cout << "ChatAdminRight,BanUsers," << int(ChatAdminRight::BanUsers) << "\n";
    std::cout << "ChatAdminRight,InviteByLinkOrAdd," << int(ChatAdminRight::InviteByLinkOrAdd) << "\n";
    std::cout << "ChatAdminRight,PinMessages," << int(ChatAdminRight::PinMessages) << "\n";
    std::cout << "ChatAdminRight,AddAdmins," << int(ChatAdminRight::AddAdmins) << "\n";
    std::cout << "ChatAdminRight,Anonymous," << int(ChatAdminRight::Anonymous) << "\n";
    std::cout << "ChatAdminRight,ManageCall," << int(ChatAdminRight::ManageCall) << "\n";
    std::cout << "ChatAdminRight,Other," << int(ChatAdminRight::Other) << "\n";
    std::cout << "ChatAdminRight,ManageTopics," << int(ChatAdminRight::ManageTopics) << "\n";
    std::cout << "ChatAdminRight,PostStories," << int(ChatAdminRight::PostStories) << "\n";
    std::cout << "ChatAdminRight,EditStories," << int(ChatAdminRight::EditStories) << "\n";
    std::cout << "ChatAdminRight,DeleteStories," << int(ChatAdminRight::DeleteStories) << "\n";
    std::cout << "ChatAdminRight,ManageDirect," << int(ChatAdminRight::ManageDirect) << "\n";
    std::cout << "ChatAdminRight,ManageRanks," << int(ChatAdminRight::ManageRanks) << "\n";
    std::cout << "ChatAdminRight,ProcessJoinRequests," << int(ChatAdminRight::ProcessJoinRequests) << "\n";
    std::cout << "ChatAdminRight,ManageLinkedPeers," << int(ChatAdminRight::ManageLinkedPeers) << "\n";
    std::cout << "ChatAdminRight,ManageWelcomeMessages," << int(ChatAdminRight::ManageWelcomeMessages) << "\n";
}
int main(int argc, char **argv) {
    if(argc == 1) { shifts(); return 0; }
    if (std::string(argv[1]) == "flags") { declarations(); return 0; }
    scenario<PeerData,Data::PeerUpdate>("peer");
    scenario<History,Data::HistoryUpdate>("history");
    scenario<Data::ForumTopic,Data::TopicUpdate>("topic");
    scenario<Data::SavedSublist,Data::SublistUpdate>("sublist");
    scenario<HistoryItem,Data::MessageUpdate>("message");
    scenario<Dialogs::Entry,Data::EntryUpdate>("entry");
    scenario<Data::Story,Data::StoryUpdate>("story");
    order();
}
