"""Extract unchanged pure bodies only; never synthesize PeerData or Session."""
from pathlib import Path
import sys,re
root,out=map(Path,sys.argv[1:3])
def read(name):return (root/'SourceFiles'/name).read_text()
def function(text,start):
    pos=text.index(start);end=text.index('\n}',pos)+2
    return text[pos:end]+'\n'
peer=read('data/data_peer.cpp'); header=read('data/data_peer.h'); birthday=read('data/data_birthday.cpp')
s='#include "prefix.hpp"\n#include "data/data_birthday.h"\n#include "data/data_lastseen_status.h"\n#include "data/data_flags.h"\n#include "data/data_user_names.h"\n#include <QtCore/QDate>\n'
s+=re.search(r'enum class PeerBarSetting \{.*?\n\};',header,re.S)[0]+'\ninline constexpr bool is_flag_type(PeerBarSetting) {return true;}\nusing PeerBarSettings=base::flags<PeerBarSetting>;\n'
s+='namespace Data {\n'+header[header.index('class RestrictionCheckResult'):header.index('struct UnavailableReason')]
s+=birthday[birthday.index('namespace {'):birthday.index('QString BirthdayText')]
for start in ['QString BirthdayCake()', 'int BirthdayAge(', 'bool IsBirthdayToday(']:s+=function(birthday,start)
s+='}\n'
for start in ['[[nodiscard]] int ParseRegistrationDate(', '[[nodiscard]] int RegistrationYear(', '[[nodiscard]] int RegistrationMonth(']:s+=function(peer,start)
s+='namespace Data {\n'+function(peer,'PeerId FakePeerIdForJustName(')+'}\n'
(out/'pure.cpp').write_text(s)

utils=read('core/utils.cpp')
a=utils.index('namespace {\n\tQMap<QString, QString> fastRusEng;')
b=utils.index('QString rusKeyboardLayoutSwitch(',a)
b=utils.index('\n}',b)+2
with (out/'pure.cpp').open('a') as f:f.write(utils[a:b]+'\n')

image=read('ui/image/image_location.cpp')
with (out/'pure.cpp').open('a') as f:
    f.write('#define private public\n#include "ui/image/image_location.h"\n#undef private\n')
    for start in ['bool StorageFileLocation::valid()', 'bool operator==(const StorageFileLocation &a,', 'ImageLocation::ImageLocation(']:f.write(function(image,start))

entity=(root/'lib_ui/ui/text/text_entity.cpp').read_text()
text_impl=(root/'lib_ui/ui/text/text.cpp').read_text()
with (out/'pure.cpp').open('a') as f:
    f.write('namespace Ui::Text {\n'+function(text_impl,'bool IsDiacritic(QChar ch)')+'}\n')
    f.write('namespace TextUtilities { namespace { using namespace Ui::Text;\n')
    f.write(function(entity,'inline QChar RemoveOneAccent(')+function(entity,'const QRegularExpression &RegExpWordSplit()'))
    f.write('}\n'+function(entity,'QString RemoveAccents(')+function(entity,'QStringList PrepareSearchWords(')+'}\n')

import re
scheme=(Path(sys.argv[3])/'scheme.cpp').read_text()
creators=['peerColor','peerColorCollectible','inputPeerColorCollectible','profileTabPosts','profileTabGifts','profileTabMedia','profileTabFiles','profileTabMusic','profileTabVoice','profileTabLinks','profileTabGifs']
methods=[]
for name in creators:
    match=re.search(r'inline static MTP\w+ new_'+name+r'\([^\n]*\) \{',scheme)
    assert match,name
    depth,end=1,match.end()
    while depth:
        depth+=(scheme[end]=='{')-(scheme[end]=='}');end+=1
    methods.append(scheme[match.start():end])
result='#include "scheme.h"\nnamespace MTP::details {class TypeCreator final {public:\n'+'\n'.join(methods)+'\n};}\n'
for name in ['peerColor','profileTab']:
    start=scheme.index('MTP'+name+'::MTP'+name+'() = default;')
    next_class=re.search(r'^MTP(?!D)\w+::MTP(?!D)\w+\(\) = default;',scheme[start+1:],re.M)
    assert next_class,name
    end=start+1+next_class.start()
    result+=scheme[start:end]
(out/'peer_scheme.cpp').write_text(result)
with(out/'pure.cpp').open('a')as f:
    f.write('namespace Data {\n'+re.search(r'enum class ProfileTab : uchar \{.*?\n\};',header,re.S)[0]+'\n')
    for start in ['uint64 BackgroundEmojiIdFromColor(', 'std::optional<uint8> ColorIndexFromColor(', 'ProfileTab ParseProfileTab(', 'MTPProfileTab ProfileTabToMTP(']:f.write(function(peer,start))
    f.write('}\n')

with(out/'pure.cpp').open('a')as f:
    f.write(re.search(r'enum class UserDataFlag[^\{]*\{.*?\n\};',read('data/data_user.h'),re.S)[0]+'\n')
    f.write(re.search(r'enum class ChatDataFlag[^\{]*\{.*?\n\};',read('data/data_chat.h'),re.S)[0]+'\n')
    f.write(re.search(r'enum class ChannelDataFlag[^\{]*\{.*?\n\};',read('data/data_channel.h'),re.S)[0]+'\n')
    f.write(re.search(r'enum class ChatRestriction[^\{]*\{.*?\n\};',read('data/data_chat_participant_status.h'),re.S)[0]+'\n')
