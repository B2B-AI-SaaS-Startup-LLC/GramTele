#include "data/data_peer_id.h"
#include "data/data_msg_id.h"
#include <sstream>

std::string wire(const MTPPeer &peer) {
    mtpBuffer data; peer.write(data);
    return QByteArray(reinterpret_cast<const char*>(data.constData()),data.size()*4).toHex().toStdString();
}
void peerCase(int kind, uint64 bits) {
    auto id = PeerId();
    auto input = std::string("-");
    switch (kind) {
    case 0: id = PeerId(bits); break;
    case 1: id = peerFromUser(UserId(bits)); break;
    case 2: id = peerFromChat(ChatId(bits)); break;
    case 3: id = peerFromChannel(ChannelId(bits)); break;
    case 4: id = PeerId(FakeChatId(bits)); break;
    case 5: id = DeserializePeerId(bits); break;
    case 6: case 7: case 8: {
        const auto peer = MTPPeer(kind == 6 ? MTP_peerUser(MTP_long(bits))
            : kind == 7 ? MTP_peerChat(MTP_long(bits)) : MTP_peerChannel(MTP_long(bits)));
        input = wire(peer); id = peerFromMTP(peer);
    } break;
    case 9: id = peerFromChat(MTP_int(static_cast<int32>(bits))); break;
    case 10: id = peerFromChannel(MTP_int(static_cast<int32>(bits))); break;
    default: std::exit(1);
    }
    std::cout << "P " << id.value << ' ' << bool(id) << ' ' << peerIsUser(id) << ' '
        << peerIsChat(id) << ' ' << peerIsChannel(id) << ' ' << peerToUser(id).bare << ' '
        << peerToChat(id).bare << ' ' << peerToChannel(id).bare << ' ' << id.to<FakeChatId>().bare
        << ' ' << peerToBareMTPInt(id).v << ' ';
    if (!(id.value & (UserId::kReservedBit << 48))) {
        const auto saved = SerializePeerId(id);
        QByteArray data; QDataStream stream(&data,QIODevice::WriteOnly); stream.setVersion(QDataStream::Qt_5_1); stream << saved;
        std::cout << saved << ' ' << data.toHex().toStdString();
    } else std::cout << "- -";
    std::cout << ' ' << wire(MTPPeer(peerToMTP(id))) << ' ' << input << '\n';
}
template <typename F>
void arithmetic(__int128 expected, F &&call) {
    if (expected < std::numeric_limits<int64>::min() || expected > std::numeric_limits<int64>::max()) std::cout << "- ";
    else std::cout << call().bare << ' ';
}
void messageCase(int64 bare, uint64 peerValue, int32 story, uint64 session, int64 other) {
    const auto id = MsgId(bare);
    const auto full = FullMsgId(PeerId(peerValue),id);
    const auto storyId = FullStoryId{PeerId(peerValue),story};
    const auto global = GlobalMsgId{full,session};
    std::cout << "M " << bool(id) << ' ' << !id << ' ' << IsClientMsgId(id) << ' '
        << IsStoryMsgId(id) << ' ' << IsServerMsgId(id) << ' ';
    if (IsClientMsgId(id)) std::cout << ClientMsgIndex(id); else std::cout << '-';
    std::cout << ' ';
    if (IsStoryMsgId(id)) std::cout << StoryIdFromMsgId(id); else std::cout << '-';
    std::cout << ' ';
    arithmetic(-__int128(bare),[&]{return -id;});
    arithmetic(__int128(bare)+other,[&]{return id+MsgId(other);});
    arithmetic(__int128(bare)-other,[&]{return id-MsgId(other);});
    arithmetic(__int128(bare)+1,[&]{auto value=id;return ++value;});
    arithmetic(__int128(bare)+1,[&]{auto value=id;return value++;});
    arithmetic(__int128(bare)-1,[&]{auto value=id;return --value;});
    arithmetic(__int128(bare)-1,[&]{auto value=id;return value--;});
    std::cout << bool(full) << ' ' << !full << ' ' << storyId.valid() << ' ' << bool(global) << ' ' << !global << ' '
        << (id < MsgId(other)) << ' ' << (id == MsgId(other)) << ' '
        << (full < FullMsgId(PeerId(0),MsgId(other))) << ' '
        << (MsgRange{id,MsgId(other)} == MsgRange{MsgId(other),id}) << '\n';
}
void constants() {
    std::cout << "C";
    for (const auto value : {StartClientMsgId.bare,ClientMsgIds,EndClientMsgId.bare,StartStoryMsgId.bare,
            int64(ServerMaxStoryId),StoryMsgIds,EndStoryMsgId.bare,ServerMaxMsgId.bare,ScheduledMaxMsgId.bare,
            ShortcutMaxMsgId.bare,WelcomeMaxMsgId.bare,ShowAtUnreadMsgId.bare,SpecialMsgIdShift,ShowAtTheEndMsgId.bare,
            SwitchAtTopMsgId.bare,ShowAndStartBotMsgId.bare,ShowAndMaybeStartBotMsgId.bare,
            ShowForChooseMessagesMsgId.bare,int64(kSearchQueryOffsetHint)}) std::cout << ' ' << value;
    std::cout << '\n';
}
int main() {
    std::string line;
    while (std::getline(std::cin,line)) {
        std::istringstream in(line); char command; in >> command;
        if (command == 'P') { int kind; uint64 bits; in >> kind >> bits; peerCase(kind,bits); }
        else if (command == 'M') { int64 bare,other; uint64 peer,session; int32 story;
            in >> bare >> peer >> story >> session >> other; messageCase(bare,peer,story,session,other); }
        else if (command == 'C') constants();
        else if (command == 'I') { int32 index; in >> index; const auto id=StoryIdToMsgId(index);
            std::cout << "I " << ClientMsgByIndex(index).bare << ' ' << id.bare << ' ' << IsStoryMsgId(id) << '\n'; }
        else return 1;
        if (!in) return 1;
    }
}
