from pathlib import Path
import sys,re
root,out,gen=map(Path,sys.argv[1:])
def read(name):return (root/'SourceFiles'/name).read_text()
def function(s,start):
 a=s.index(start);return s[a:s.index('\n}',a)+2]+'\n'
s=read('data/data_chat_participant_status.cpp')
pure='#include "data/data_chat_participant_status.h"\n'+s[s.index('namespace {'):s.index('namespace Data {')]
pure+='namespace Data {\n'
for name in ['AllSendRestrictions','FilesSendRestrictions','TabbedPanelSendRestrictions']:pure+=function(s,'ChatRestrictions '+name+'()')
pure+='}\n'
a=read('ayu/utils/telegram_helpers.cpp');pure+=a[a.index('const auto kZalgoPattern'):a.index('\n\n',a.index('const auto kZalgoPattern'))]+'\n'+function(a,'QString filterZalgo(')
(out/'pure.cpp').write_text(pure)
scheme=(gen/'scheme.cpp').read_text();methods=[]
for name in ['chatAdminRights','chatBannedRights']:
 m=re.search(r'inline static MTP\w+ new_'+name+r'\([^\n]*\) \{',scheme);assert m
 depth,end=1,m.end()
 while depth:depth+=(scheme[end]=='{')-(scheme[end]=='}');end+=1
 methods.append(scheme[m.start():end])
r='#include "scheme.h"\nnamespace MTP::details {class TypeCreator final {public:\n'+'\n'.join(methods)+'\n};}\n'
for name in ['chatAdminRights','chatBannedRights']:
 start=scheme.index('MTP'+name+'::MTP'+name+'()')
 end=re.search(r'^MTP(?!D)\w+::MTP(?!D)\w+\(\)',scheme[start+1:],re.M);assert end
 r+=scheme[start:start+1+end.start()]
(out/'scheme_rights.cpp').write_text(r)
header=read('data/data_channel.h');chat=read('data/data_chat.h');channel=read('data/data_channel.cpp')
with(out/'pure.cpp').open('a')as f:
 for name,s in [('ChannelDataFlag',header),('ChatDataFlag',chat)]:
  f.write(re.search(r'enum class '+name+r'[^\{]*\{.*?\n\};',s,re.S)[0]+'\n')
  f.write('inline constexpr bool is_flag_type('+name+') {return true;}\n')
 f.write('using ChannelDataFlags=base::flags<ChannelDataFlag>;\nusing Flag=ChannelDataFlag;\n')
 a=channel.index('if (which & Flag::MonoforumAdmin)',channel.index('void ChannelData::setFlags'))
 b=channel.index('\n\tconst auto diff',a)
 f.write('ChannelDataFlags NormalizeFlags(ChannelDataFlags which) {\n'+channel[a:b]+'\nreturn which;\n}\n')
 f.write('static constexpr auto kRestrictUntilForever=TimeId(INT_MAX);\n'+re.search(r'\[\[nodiscard\]\] static bool IsRestrictedForever\(.*?\n\t\}',header,re.S)[0])
