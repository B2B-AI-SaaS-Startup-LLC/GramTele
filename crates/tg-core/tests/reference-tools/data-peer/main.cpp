#include "pure.cpp"
static QDate today(2026,11,14);
extern "C" QDate __wrap__ZN5QDate11currentDateEv() { return today; }
int main() {
    for (int y : {0,1874,1875,1900,2000,2024,2100,2101}) for(int m=-1;m!=14;++m) for(int d=0;d!=33;++d){
        auto b=Data::Birthday(d,m,y);std::cout<<"b,"<<d<<','<<m<<','<<y<<','<<b.serialize()<<','<<b.valid()<<','<<b.day()<<','<<b.month()<<','<<b.year()<<'\n';
    }
    for (auto now : {QDate(2023,1,1),QDate(2024,2,28),QDate(2024,2,29),QDate(2026,11,14)}) {
        today=now;
        for(auto b : {Data::Birthday(),Data::Birthday(29,2,2000),Data::Birthday(14,11,2026),Data::Birthday(15,11,2000),Data::Birthday(1,1,0)})
        std::cout<<"age,"<<now.year()<<','<<now.month()<<','<<now.day()<<','<<b.serialize()<<','<<Data::BirthdayAge(b)<<','<<Data::IsBirthdayToday(b)<<'\n';
    }
    for(uint32 raw : {0U,1U,2U,3U,4U,0x3fffffffU,0x40000000U,0x40000004U,0x7fffffffU,0x80000000U,0xffffffffU,0x6e03b5fbU}) {
        auto a=Data::LastseenStatus::FromSerialized(raw);
        std::cout<<"l,"<<raw<<','<<a.serialize()<<','<<a.isHidden()<<','<<a.isRecently()<<','<<a.isWithinWeek()<<','<<a.isWithinMonth()<<','<<a.isLongAgo()<<','<<a.isHiddenByMe()<<','<<a.isOnline(0)<<','<<a.isOnline(-1)<<','<<a.isLocalOnlineValue()<<','<<a.onlineTill()<<'\n';
    }
    for(int value:{-1375315199,-100000,-31,-30,-4,-3,-2,-1,0,1,1375315203,1375315204,2000000000,2147483647}) {
        for(bool local:{false,true})for(bool hidden:{false,true}) {
            auto a=Data::LastseenStatus::OnlineTill(value,local,hidden);
            std::cout<<"o,"<<value<<','<<local<<','<<hidden<<','<<a.serialize()<<'\n';
        }
        auto a=Data::LastseenStatus::FromLegacy(value);std::cout<<"legacy,"<<value<<','<<a.serialize()<<'\n';
    }
    for(auto raw:{0ULL,777000ULL,0xffffffffffffffffULL}) {
        const auto id=PeerId(raw);std::cout<<"p,"<<raw<<','<<peerIsUser(id)<<','<<peerIsChat(id)<<','<<peerIsChannel(id)<<','<<(peerIsUser(id)&&!(raw%1000))<<'\n';
    }
    for(auto date:{QString("01.2013"),QString("12.2099"),QString("00.2020"),QString("+1.2020"),QString("01.2100"),QString(" 1.2020"),QString("01. 020")}) std::cout<<"reg,"<<date.toStdString()<<','<<ParseRegistrationDate(date)<<'\n';
    Data::Flags<PeerBarSettings,3> flags;
    rpl::lifetime life;
    flags.value() | rpl::on_next([](auto c){std::cout<<"f,"<<c.diff.value()<<','<<c.value.value()<<'\n';},life);
    for(auto raw:{4,5,5,7,6,2,0})flags.set(PeerBarSettings::from_raw(raw));
    flags.add(PeerBarSetting::ReportSpam);flags.add(PeerBarSetting::ReportSpam);flags.remove(PeerBarSetting::ReportSpam);
    Data::UsernamesInfo names;
    names.setUsernames({{"first",true,false},{"edit",true,true}});
    names.setUsername({});names.setUsername("appended");
    std::cout<<"names,"<<names.username().toStdString()<<','<<names.editableUsername().toStdString()<<','<<names.usernames().size()<<'\n';
    for(auto text : {QString::fromUtf8("Александр"),QString::fromUtf8("Пётр"),QString::fromUtf8("ИЙ ия Ый ъЬь"),QString::fromUtf8("Филипп Ильин"),QString("qwerty[];'"),QString::fromUtf8("Привет ґїіє"),QString::fromUtf8("😀 École 東京")}) {
        std::cout<<"search,"<<text.toUtf8().toHex().toStdString()<<','<<translitRusEng(text).toUtf8().toHex().toStdString()<<','<<rusKeyboardLayoutSwitch(text).toUtf8().toHex().toStdString()<<'\n';
        std::cout<<"fake,"<<text.toUtf8().toHex().toStdString()<<','<<Data::FakePeerIdForJustName(text).value<<'\n';
    }

    for (auto r : {Data::RestrictionCheckResult::Allowed(),Data::RestrictionCheckResult::WithEveryone(),Data::RestrictionCheckResult::Explicit()}) std::cout<<"restriction,"<<bool(r)<<','<<r.isAllowed()<<','<<r.isWithEveryone()<<','<<r.isExplicit()<<'\n';
    StorageFileLocation a,b;
    a._dcId=1;a._type=StorageFileLocation::Type::PeerPhoto;a._id=42;a._volumeId=7;a._sizeLetter='a';b=a;b._accessHash=99;b._fileReference="changed";
    ImageLocation ia({a},160,160),ib({b},320,320);
    std::cout<<"image,"<<a.valid()<<','<<(a==b)<<','<<(ia==ib)<<','<<ia.width()<<','<<ib.height()<<'\n';
    b._volumeId=8;std::cout<<"imageDifferent,"<<(a==b)<<'\n';
    for(auto text:{QString("spambot"),QString("SPAMBOT"),QString::fromUtf8("ſpambot"),QString("other")})std::cout<<"freeze,"<<text.toUtf8().toHex().toStdString()<<','<<(text.compare(u"spambot"_q,Qt::CaseInsensitive)==0)<<'\n';

    for(auto input : {QString::fromUtf8("École 東京"),QString::fromUtf8("Сергей"),QString("Saved messages"),QString::fromUtf8("Сохранённое"),QString("abc@User-Name_1"),QString::fromUtf8("a\xCC\x81 😀")}) {
        const auto clean=TextUtilities::RemoveAccents(input);
        const auto words=TextUtilities::PrepareSearchWords(input,nullptr);
        std::cout<<"normalization,"<<input.toUtf8().toHex().toStdString()<<','<<clean.toUtf8().toHex().toStdString()<<',';
        bool first=true;for(const auto &word:words){if(!first)std::cout<<';';first=false;std::cout<<word.toUtf8().toHex().toStdString();}std::cout<<'\n';
    }

    for(int i=1;i!=9;++i){const auto tab=Data::ProfileTabToMTP(static_cast<Data::ProfileTab>(i));std::cout<<"profile,"<<i<<','<<uint32(tab.type())<<','<<int(Data::ParseProfileTab(&tab))<<'\n';}
    for(int i : {-1,0,1,255,256,2147483647}) {
        const MTPPeerColor color=MTP_peerColor(MTP_flags(MTPDpeerColor::Flag::f_color|MTPDpeerColor::Flag::f_background_emoji_id),MTP_int(i),MTP_long(42));
        std::cout<<"color,"<<i<<','<<int(*Data::ColorIndexFromColor(&color))<<','<<Data::BackgroundEmojiIdFromColor(&color)<<'\n';
    }
    std::cout<<"flag,UserDataFlag,Contact,"<<uint64(UserDataFlag::Contact)<<"\n";
    std::cout<<"flag,UserDataFlag,MutualContact,"<<uint64(UserDataFlag::MutualContact)<<"\n";
    std::cout<<"flag,UserDataFlag,Deleted,"<<uint64(UserDataFlag::Deleted)<<"\n";
    std::cout<<"flag,UserDataFlag,Verified,"<<uint64(UserDataFlag::Verified)<<"\n";
    std::cout<<"flag,UserDataFlag,Scam,"<<uint64(UserDataFlag::Scam)<<"\n";
    std::cout<<"flag,UserDataFlag,Fake,"<<uint64(UserDataFlag::Fake)<<"\n";
    std::cout<<"flag,UserDataFlag,BotInlineGeo,"<<uint64(UserDataFlag::BotInlineGeo)<<"\n";
    std::cout<<"flag,UserDataFlag,Blocked,"<<uint64(UserDataFlag::Blocked)<<"\n";
    std::cout<<"flag,UserDataFlag,HasPhoneCalls,"<<uint64(UserDataFlag::HasPhoneCalls)<<"\n";
    std::cout<<"flag,UserDataFlag,PhoneCallsPrivate,"<<uint64(UserDataFlag::PhoneCallsPrivate)<<"\n";
    std::cout<<"flag,UserDataFlag,Support,"<<uint64(UserDataFlag::Support)<<"\n";
    std::cout<<"flag,UserDataFlag,CanPinMessages,"<<uint64(UserDataFlag::CanPinMessages)<<"\n";
    std::cout<<"flag,UserDataFlag,DiscardMinPhoto,"<<uint64(UserDataFlag::DiscardMinPhoto)<<"\n";
    std::cout<<"flag,UserDataFlag,Self,"<<uint64(UserDataFlag::Self)<<"\n";
    std::cout<<"flag,UserDataFlag,Premium,"<<uint64(UserDataFlag::Premium)<<"\n";
    std::cout<<"flag,UserDataFlag,UnofficialSecurityRisk,"<<uint64(UserDataFlag::UnofficialSecurityRisk)<<"\n";
    std::cout<<"flag,UserDataFlag,VoiceMessagesForbidden,"<<uint64(UserDataFlag::VoiceMessagesForbidden)<<"\n";
    std::cout<<"flag,UserDataFlag,PersonalPhoto,"<<uint64(UserDataFlag::PersonalPhoto)<<"\n";
    std::cout<<"flag,UserDataFlag,StoriesHidden,"<<uint64(UserDataFlag::StoriesHidden)<<"\n";
    std::cout<<"flag,UserDataFlag,HasActiveStories,"<<uint64(UserDataFlag::HasActiveStories)<<"\n";
    std::cout<<"flag,UserDataFlag,HasUnreadStories,"<<uint64(UserDataFlag::HasUnreadStories)<<"\n";
    std::cout<<"flag,UserDataFlag,RequiresPremiumToWrite,"<<uint64(UserDataFlag::RequiresPremiumToWrite)<<"\n";
    std::cout<<"flag,UserDataFlag,HasRequirePremiumToWrite,"<<uint64(UserDataFlag::HasRequirePremiumToWrite)<<"\n";
    std::cout<<"flag,UserDataFlag,HasStarsPerMessage,"<<uint64(UserDataFlag::HasStarsPerMessage)<<"\n";
    std::cout<<"flag,UserDataFlag,MessageMoneyRestrictionsKnown,"<<uint64(UserDataFlag::MessageMoneyRestrictionsKnown)<<"\n";
    std::cout<<"flag,UserDataFlag,ReadDatesPrivate,"<<uint64(UserDataFlag::ReadDatesPrivate)<<"\n";
    std::cout<<"flag,UserDataFlag,StoriesCorrespondent,"<<uint64(UserDataFlag::StoriesCorrespondent)<<"\n";
    std::cout<<"flag,UserDataFlag,Forum,"<<uint64(UserDataFlag::Forum)<<"\n";
    std::cout<<"flag,UserDataFlag,HasActiveVideoStream,"<<uint64(UserDataFlag::HasActiveVideoStream)<<"\n";
    std::cout<<"flag,UserDataFlag,NoForwardsMyEnabled,"<<uint64(UserDataFlag::NoForwardsMyEnabled)<<"\n";
    std::cout<<"flag,UserDataFlag,NoForwardsPeerEnabled,"<<uint64(UserDataFlag::NoForwardsPeerEnabled)<<"\n";
    std::cout<<"flag,ChatDataFlag,Left,"<<uint64(ChatDataFlag::Left)<<"\n";
    std::cout<<"flag,ChatDataFlag,Creator,"<<uint64(ChatDataFlag::Creator)<<"\n";
    std::cout<<"flag,ChatDataFlag,Deactivated,"<<uint64(ChatDataFlag::Deactivated)<<"\n";
    std::cout<<"flag,ChatDataFlag,Forbidden,"<<uint64(ChatDataFlag::Forbidden)<<"\n";
    std::cout<<"flag,ChatDataFlag,CallActive,"<<uint64(ChatDataFlag::CallActive)<<"\n";
    std::cout<<"flag,ChatDataFlag,CallNotEmpty,"<<uint64(ChatDataFlag::CallNotEmpty)<<"\n";
    std::cout<<"flag,ChatDataFlag,CanSetUsername,"<<uint64(ChatDataFlag::CanSetUsername)<<"\n";
    std::cout<<"flag,ChatDataFlag,NoForwards,"<<uint64(ChatDataFlag::NoForwards)<<"\n";
    std::cout<<"flag,ChatDataFlag,HasWelcomeMessages,"<<uint64(ChatDataFlag::HasWelcomeMessages)<<"\n";
    std::cout<<"flag,ChatDataFlag,AyuNoForwards,"<<uint64(ChatDataFlag::AyuNoForwards)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Left,"<<uint64(ChannelDataFlag::Left)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Creator,"<<uint64(ChannelDataFlag::Creator)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Forbidden,"<<uint64(ChannelDataFlag::Forbidden)<<"\n";
    std::cout<<"flag,ChannelDataFlag,CallActive,"<<uint64(ChannelDataFlag::CallActive)<<"\n";
    std::cout<<"flag,ChannelDataFlag,CallNotEmpty,"<<uint64(ChannelDataFlag::CallNotEmpty)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Signatures,"<<uint64(ChannelDataFlag::Signatures)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Verified,"<<uint64(ChannelDataFlag::Verified)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Scam,"<<uint64(ChannelDataFlag::Scam)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Fake,"<<uint64(ChannelDataFlag::Fake)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Megagroup,"<<uint64(ChannelDataFlag::Megagroup)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Broadcast,"<<uint64(ChannelDataFlag::Broadcast)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Gigagroup,"<<uint64(ChannelDataFlag::Gigagroup)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Username,"<<uint64(ChannelDataFlag::Username)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Location,"<<uint64(ChannelDataFlag::Location)<<"\n";
    std::cout<<"flag,ChannelDataFlag,CanSetUsername,"<<uint64(ChannelDataFlag::CanSetUsername)<<"\n";
    std::cout<<"flag,ChannelDataFlag,CanSetStickers,"<<uint64(ChannelDataFlag::CanSetStickers)<<"\n";
    std::cout<<"flag,ChannelDataFlag,PreHistoryHidden,"<<uint64(ChannelDataFlag::PreHistoryHidden)<<"\n";
    std::cout<<"flag,ChannelDataFlag,CanViewParticipants,"<<uint64(ChannelDataFlag::CanViewParticipants)<<"\n";
    std::cout<<"flag,ChannelDataFlag,HasLink,"<<uint64(ChannelDataFlag::HasLink)<<"\n";
    std::cout<<"flag,ChannelDataFlag,SlowmodeEnabled,"<<uint64(ChannelDataFlag::SlowmodeEnabled)<<"\n";
    std::cout<<"flag,ChannelDataFlag,NoForwards,"<<uint64(ChannelDataFlag::NoForwards)<<"\n";
    std::cout<<"flag,ChannelDataFlag,JoinToWrite,"<<uint64(ChannelDataFlag::JoinToWrite)<<"\n";
    std::cout<<"flag,ChannelDataFlag,RequestToJoin,"<<uint64(ChannelDataFlag::RequestToJoin)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Forum,"<<uint64(ChannelDataFlag::Forum)<<"\n";
    std::cout<<"flag,ChannelDataFlag,AntiSpam,"<<uint64(ChannelDataFlag::AntiSpam)<<"\n";
    std::cout<<"flag,ChannelDataFlag,ParticipantsHidden,"<<uint64(ChannelDataFlag::ParticipantsHidden)<<"\n";
    std::cout<<"flag,ChannelDataFlag,StoriesHidden,"<<uint64(ChannelDataFlag::StoriesHidden)<<"\n";
    std::cout<<"flag,ChannelDataFlag,HasActiveStories,"<<uint64(ChannelDataFlag::HasActiveStories)<<"\n";
    std::cout<<"flag,ChannelDataFlag,HasUnreadStories,"<<uint64(ChannelDataFlag::HasUnreadStories)<<"\n";
    std::cout<<"flag,ChannelDataFlag,CanGetStatistics,"<<uint64(ChannelDataFlag::CanGetStatistics)<<"\n";
    std::cout<<"flag,ChannelDataFlag,ViewAsMessages,"<<uint64(ChannelDataFlag::ViewAsMessages)<<"\n";
    std::cout<<"flag,ChannelDataFlag,SimilarExpanded,"<<uint64(ChannelDataFlag::SimilarExpanded)<<"\n";
    std::cout<<"flag,ChannelDataFlag,CanViewRevenue,"<<uint64(ChannelDataFlag::CanViewRevenue)<<"\n";
    std::cout<<"flag,ChannelDataFlag,PaidMediaAllowed,"<<uint64(ChannelDataFlag::PaidMediaAllowed)<<"\n";
    std::cout<<"flag,ChannelDataFlag,CanViewCreditsRevenue,"<<uint64(ChannelDataFlag::CanViewCreditsRevenue)<<"\n";
    std::cout<<"flag,ChannelDataFlag,SignatureProfiles,"<<uint64(ChannelDataFlag::SignatureProfiles)<<"\n";
    std::cout<<"flag,ChannelDataFlag,StargiftsAvailable,"<<uint64(ChannelDataFlag::StargiftsAvailable)<<"\n";
    std::cout<<"flag,ChannelDataFlag,PaidMessagesAvailable,"<<uint64(ChannelDataFlag::PaidMessagesAvailable)<<"\n";
    std::cout<<"flag,ChannelDataFlag,AutoTranslation,"<<uint64(ChannelDataFlag::AutoTranslation)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Monoforum,"<<uint64(ChannelDataFlag::Monoforum)<<"\n";
    std::cout<<"flag,ChannelDataFlag,MonoforumAdmin,"<<uint64(ChannelDataFlag::MonoforumAdmin)<<"\n";
    std::cout<<"flag,ChannelDataFlag,MonoforumDisabled,"<<uint64(ChannelDataFlag::MonoforumDisabled)<<"\n";
    std::cout<<"flag,ChannelDataFlag,ForumTabs,"<<uint64(ChannelDataFlag::ForumTabs)<<"\n";
    std::cout<<"flag,ChannelDataFlag,HasStarsPerMessage,"<<uint64(ChannelDataFlag::HasStarsPerMessage)<<"\n";
    std::cout<<"flag,ChannelDataFlag,StarsPerMessageKnown,"<<uint64(ChannelDataFlag::StarsPerMessageKnown)<<"\n";
    std::cout<<"flag,ChannelDataFlag,HasActiveVideoStream,"<<uint64(ChannelDataFlag::HasActiveVideoStream)<<"\n";
    std::cout<<"flag,ChannelDataFlag,Community,"<<uint64(ChannelDataFlag::Community)<<"\n";
    std::cout<<"flag,ChannelDataFlag,CommunityCollapsed,"<<uint64(ChannelDataFlag::CommunityCollapsed)<<"\n";
    std::cout<<"flag,ChannelDataFlag,HasWelcomeMessages,"<<uint64(ChannelDataFlag::HasWelcomeMessages)<<"\n";
    std::cout<<"flag,ChannelDataFlag,AyuNoForwards,"<<uint64(ChannelDataFlag::AyuNoForwards)<<"\n";
    std::cout<<"flag,ChatRestriction,ViewMessages,"<<uint64(ChatRestriction::ViewMessages)<<"\n";
    std::cout<<"flag,ChatRestriction,SendStickers,"<<uint64(ChatRestriction::SendStickers)<<"\n";
    std::cout<<"flag,ChatRestriction,SendGifs,"<<uint64(ChatRestriction::SendGifs)<<"\n";
    std::cout<<"flag,ChatRestriction,SendGames,"<<uint64(ChatRestriction::SendGames)<<"\n";
    std::cout<<"flag,ChatRestriction,SendInline,"<<uint64(ChatRestriction::SendInline)<<"\n";
    std::cout<<"flag,ChatRestriction,SendPolls,"<<uint64(ChatRestriction::SendPolls)<<"\n";
    std::cout<<"flag,ChatRestriction,SendPhotos,"<<uint64(ChatRestriction::SendPhotos)<<"\n";
    std::cout<<"flag,ChatRestriction,SendVideos,"<<uint64(ChatRestriction::SendVideos)<<"\n";
    std::cout<<"flag,ChatRestriction,SendVideoMessages,"<<uint64(ChatRestriction::SendVideoMessages)<<"\n";
    std::cout<<"flag,ChatRestriction,SendMusic,"<<uint64(ChatRestriction::SendMusic)<<"\n";
    std::cout<<"flag,ChatRestriction,SendVoiceMessages,"<<uint64(ChatRestriction::SendVoiceMessages)<<"\n";
    std::cout<<"flag,ChatRestriction,SendFiles,"<<uint64(ChatRestriction::SendFiles)<<"\n";
    std::cout<<"flag,ChatRestriction,SendOther,"<<uint64(ChatRestriction::SendOther)<<"\n";
    std::cout<<"flag,ChatRestriction,EmbedLinks,"<<uint64(ChatRestriction::EmbedLinks)<<"\n";
    std::cout<<"flag,ChatRestriction,ChangeInfo,"<<uint64(ChatRestriction::ChangeInfo)<<"\n";
    std::cout<<"flag,ChatRestriction,AddParticipants,"<<uint64(ChatRestriction::AddParticipants)<<"\n";
    std::cout<<"flag,ChatRestriction,PinMessages,"<<uint64(ChatRestriction::PinMessages)<<"\n";
    std::cout<<"flag,ChatRestriction,CreateTopics,"<<uint64(ChatRestriction::CreateTopics)<<"\n";
    std::cout<<"flag,ChatRestriction,EditRank,"<<uint64(ChatRestriction::EditRank)<<"\n";
    std::cout<<"flag,ChatRestriction,SendReactions,"<<uint64(ChatRestriction::SendReactions)<<"\n";
    std::cout<<"flag,ChatRestriction,ManageLinkedPeers,"<<uint64(ChatRestriction::ManageLinkedPeers)<<"\n";
    std::cout<<"flag,PeerBarSetting,ReportSpam,"<<uint64(PeerBarSetting::ReportSpam)<<"\n";
    std::cout<<"flag,PeerBarSetting,AddContact,"<<uint64(PeerBarSetting::AddContact)<<"\n";
    std::cout<<"flag,PeerBarSetting,BlockContact,"<<uint64(PeerBarSetting::BlockContact)<<"\n";
    std::cout<<"flag,PeerBarSetting,ShareContact,"<<uint64(PeerBarSetting::ShareContact)<<"\n";
    std::cout<<"flag,PeerBarSetting,NeedContactsException,"<<uint64(PeerBarSetting::NeedContactsException)<<"\n";
    std::cout<<"flag,PeerBarSetting,AutoArchived,"<<uint64(PeerBarSetting::AutoArchived)<<"\n";
    std::cout<<"flag,PeerBarSetting,RequestChat,"<<uint64(PeerBarSetting::RequestChat)<<"\n";
    std::cout<<"flag,PeerBarSetting,RequestChatIsBroadcast,"<<uint64(PeerBarSetting::RequestChatIsBroadcast)<<"\n";
    std::cout<<"flag,PeerBarSetting,HasBusinessBot,"<<uint64(PeerBarSetting::HasBusinessBot)<<"\n";
    std::cout<<"flag,PeerBarSetting,BusinessBotPaused,"<<uint64(PeerBarSetting::BusinessBotPaused)<<"\n";
    std::cout<<"flag,PeerBarSetting,BusinessBotCanReply,"<<uint64(PeerBarSetting::BusinessBotCanReply)<<"\n";
    std::cout<<"flag,PeerBarSetting,Unknown,"<<uint64(PeerBarSetting::Unknown)<<"\n";

}
