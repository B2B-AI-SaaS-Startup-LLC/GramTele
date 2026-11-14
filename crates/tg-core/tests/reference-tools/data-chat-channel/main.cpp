#include <QtCore/QRegularExpression>
#include "pure.cpp"
int main(){
 for(uint32 raw : {0U,1U,0xffffffffU,0x55555555U,0xaaaaaaaaU}) {
  auto a=ChatAdminRightsInfo(MTP_chatAdminRights(MTP_flags(MTPDchatAdminRights::Flags::from_raw(raw))));
  auto b=ChatRestrictionsInfo(MTP_chatBannedRights(MTP_flags(MTPDchatBannedRights::Flags::from_raw(raw)),MTP_int(-1)));
  std::cout<<"rights,"<<raw<<','<<a.flags.value()<<','<<AdminRightsToMTP(a).c_chatAdminRights().vflags().v<<','<<b.flags.value()<<','<<RestrictionsToMTP(b).c_chatBannedRights().vflags().v<<'\n';
 }
 for(int bit=0;bit!=32;++bit) {
  const auto raw=uint32(1)<<bit;
  auto a=ChatAdminRightsInfo(MTP_chatAdminRights(MTP_flags(MTPDchatAdminRights::Flags::from_raw(raw))));
  auto b=ChatRestrictionsInfo(MTP_chatBannedRights(MTP_flags(MTPDchatBannedRights::Flags::from_raw(raw)),MTP_int(123)));
  std::cout<<"rights,"<<raw<<','<<a.flags.value()<<','<<AdminRightsToMTP(a).c_chatAdminRights().vflags().v<<','<<b.flags.value()<<','<<RestrictionsToMTP(b).c_chatBannedRights().vflags().v<<'\n';
 }
 std::cout<<"send,"<<Data::AllSendRestrictions().value()<<','<<Data::FilesSendRestrictions().value()<<','<<Data::TabbedPanelSendRestrictions().value()<<'\n';
 const QRegularExpression mn("\\A\\p{Mn}\\z",QRegularExpression::UseUnicodePropertiesOption);
 int first=-1,last=-1;
 for(char32_t c=0;c<=0x10ffff;++c){if(c>=0xd800&&c<=0xdfff)continue;const auto s=QString::fromUcs4(&c,1);if(mn.match(s).hasMatch()){if(first<0)first=c;last=c;}else if(first>=0){std::cout<<"mn,"<<first<<','<<last<<'\n';first=-1;}}
 if(first>=0)std::cout<<"mn,"<<first<<','<<last<<'\n';
 for(auto s:{QString::fromUtf8("abc"),QString::fromUtf8("á̂"),QString::fromUtf8("á̂̃"),QString::fromUtf8("á̂̃b́̂"),QString::fromUtf8("a\u202Eb\u2069c"),QString::fromUtf8("a\U0001D167\U0001D168\U0001D169z")})std::cout<<"zalgo,"<<s.toUtf8().toHex().toStdString()<<','<<filterZalgo(s).toUtf8().toHex().toStdString()<<'\n';
 for(int bit=0;bit!=64;++bit){uint64 raw=uint64(1)<<bit;std::cout<<"normalize,"<<raw<<','<<NormalizeFlags(ChannelDataFlags::from_raw(raw)).value()<<'\n';}
 for(uint64 raw:{(uint64(1)<<23)|(uint64(1)<<39),(uint64(1)<<40)|(uint64(1)<<63),~uint64(0)})std::cout<<"normalize,"<<raw<<','<<NormalizeFlags(ChannelDataFlags::from_raw(raw)).value()<<'\n';
 for(int until:{-1,0,1,2147483646,2147483647})std::cout<<"forever,"<<until<<','<<IsRestrictedForever(until)<<'\n';
}
