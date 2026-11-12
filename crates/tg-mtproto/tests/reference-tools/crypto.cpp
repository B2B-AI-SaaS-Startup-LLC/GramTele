#include "mtproto/mtproto_auth_key.h"
#include "mtproto/details/mtproto_rsa_public_key.h"
#include <cstdio>
#include "mtproto/mtproto_config.h"
int main(int argc,char**argv) {
 if(argc<2||argc>4)return 1;FILE*f=std::fopen(argv[1],"wb");if(!f)return 2;
 MTP::AuthKey::Data data;for(unsigned i=0;i<data.size();++i)data[i]=gsl::byte(i);
 const auto key=MTP::AuthKey(data);const auto id=key.keyId();std::fwrite(&id,8,1,f);
 MTPint128 msg;for(unsigned i=0;i<16;++i)reinterpret_cast<unsigned char*>(&msg)[i]=0xf0+i;
 for(bool old:{false,true})for(bool send:{true,false}){MTPint256 k,iv;if(old)key.prepareAES_oldmtp(msg,k,iv,send);else key.prepareAES(msg,k,iv,send);std::fwrite(&k,32,1,f);std::fwrite(&iv,32,1,f);}
 const auto n=bytes::vector(256,gsl::byte(0xff));const auto e=bytes::vector{gsl::byte(1),gsl::byte(0),gsl::byte(1)};
 const auto rsa=MTP::details::RSAPublicKey(n,e);const auto fp=rsa.fingerprint();std::fwrite(&fp,8,1,f);
 auto input=bytes::vector(256,gsl::byte(0));for(unsigned i=1;i<256;++i)input[i]=gsl::byte(i);
 const auto encrypted=rsa.encrypt(input);if(encrypted.size()!=256)return 3;std::fwrite(encrypted.data(),1,encrypted.size(),f);
 for(auto environment:{MTP::Environment::Production,MTP::Environment::Test}) {
  auto options=MTP::DcOptions(environment);const auto serialized=options.serialize();uint32_t size=serialized.size();std::fwrite(&size,4,1,f);std::fwrite(serialized.data(),1,size,f);
  auto config=MTP::Config(environment);const auto bytes=config.serialize();size=bytes.size();std::fwrite(&size,4,1,f);std::fwrite(bytes.data(),1,size,f);
 }
 if(argc>3){FILE*out=std::fopen(argv[3],"wb");if(!out)return 5;unsigned char k[32],iv[32],plain[64],encrypted[64];for(unsigned i=0;i<32;++i){k[i]=i;iv[i]=i+32;}for(unsigned i=0;i<64;++i)plain[i]=i;MTP::aesIgeEncryptRaw(plain,encrypted,64,k,iv);std::fwrite(encrypted,1,64,out);std::fclose(out);}
 std::fclose(f);
 if(argc>2){FILE*out=std::fopen(argv[2],"wb");if(!out)return 4;for(auto environment:{MTP::Environment::Production,MTP::Environment::Test}){
  auto config=MTP::Config(environment);auto &fields=const_cast<MTP::ConfigFields&>(config.values());
 fields.chatSizeMax = 101;
 fields.megagroupSizeMax = 102;
 fields.forwardedCountMax = 103;
 fields.onlineUpdatePeriod = 104;
 fields.offlineBlurTimeout = 105;
 fields.offlineIdleTimeout = 106;
 fields.onlineFocusTimeout = 107;
 fields.onlineCloudTimeout = 108;
 fields.notifyCloudDelay = 109;
 fields.notifyDefaultDelay = 110;
 fields.editTimeLimit = 111;
 fields.revokeTimeLimit = 112;
 fields.revokePrivateTimeLimit = 113;
 fields.revokePrivateInbox = true;
 fields.stickersRecentLimit = 115;
 fields.internalLinksDomain = QString::fromUtf8("internalLinksDomain_😀");
 fields.channelsReadMediaPeriod = 117;
 fields.callReceiveTimeoutMs = 118;
 fields.callRingTimeoutMs = 119;
 fields.callConnectTimeoutMs = 120;
 fields.callPacketTimeoutMs = 121;
 fields.webFileDcId = 122;
 fields.txtDomainString = QString::fromUtf8("txtDomainString_😀");
 fields.blockedMode = true;
 fields.captionLengthMax = 125;
 fields.ratingDecay = 126;
 fields.reactionDefaultEmoji = QString::fromUtf8("reactionDefaultEmoji_😀");
 fields.reactionDefaultCustom = 0xfedcba9876543210ULL;
 fields.autologinToken = QString::fromUtf8("autologinToken_😀");
 fields.gifSearchUsername = QString::fromUtf8("gifSearchUsername_😀");
 fields.venueSearchUsername = QString::fromUtf8("venueSearchUsername_😀");

  const auto bytes=config.serialize();uint32_t size=bytes.size();std::fwrite(&size,4,1,out);std::fwrite(bytes.data(),1,size,out);
  fields.internalLinksDomain=QString();fields.txtDomainString=QString();fields.reactionDefaultEmoji=QString();fields.gifSearchUsername=QString();fields.venueSearchUsername=QString();
  const auto nulls=config.serialize();size=nulls.size();std::fwrite(&size,4,1,out);std::fwrite(nulls.data(),1,size,out);

 }std::fclose(out);}

}
