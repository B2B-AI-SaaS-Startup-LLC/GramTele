"""Original handleReceived envelope checks, stopping before message dispatch."""
from pathlib import Path
import sys
root=Path(sys.argv[1]);out=Path(sys.argv[2])
s=(root/'SourceFiles/mtproto/session_private.cpp').read_text()
compare=s[s.index('[[nodiscard]] bool ConstTimeIsDifferent('):s.index('\nbase::options::toggle')]
a=s.index('void SessionPrivate::handleReceived()');b=s.index('\n\t\tbool wasConnected',a)
method=s[a:b]+r'''
        accepted=true;badTime=badTime; // replaced below to avoid shadowing
        output=mtpBuffer(from_placeholder); // replaced below
    }
}
'''
method=method.replace('accepted=true;badTime=badTime; // replaced below to avoid shadowing','accepted=true;bad_time=badTime;')
method=method.replace('output=mtpBuffer(from_placeholder); // replaced below','output.resize(8 + messageLength/4);std::memcpy(output.data(),decryptedInts,output.size()*4);')
prefix=r'''
#include "mtproto/mtproto_auth_key.h"
#include <openssl/sha.h>
#include <deque>
#include <cstdio>
namespace Logs { bool DebugEnabled(){return false;} }
namespace base::unixtime {int32 now(){return 1700000000;}}
namespace MTP::details {
constexpr auto kMaxMessageLength=16*1024*1024,kIntSize=4;
class Connection {public:std::deque<mtpBuffer> queue;auto &received(){return queue;}template<class T>void logInfo(T){};};
class SessionPrivate {public:
 AuthKeyPtr _encryptionKey;uint64 _keyId=0,_sessionId=0x8877665544332211ULL;
 Connection *_connection=nullptr;bool accepted=false,bad_time=false;mtpBuffer output;
 void onReceivedSome(){}void restart(){}void handleReceived();
};
'''
suffix=r'''
}
int main(int argc,char**argv){
 if(argc!=2)return 1;auto*f=std::fopen(argv[1],"wb");if(!f)return 2;
 MTP::AuthKey::Data bytes;for(unsigned i=0;i<256;++i)bytes[i]=gsl::byte(i);
 auto auth=std::make_shared<MTP::AuthKey>(bytes);
 auto make=[&](int variant){
  mtpBuffer plain(12);auto put64=[&](int at,uint64 v){std::memcpy(plain.data()+at,&v,8);};
  put64(0,0x1122334455667788ULL);put64(2,0x8877665544332211ULL);put64(4,(1700000000ULL<<32)|1);
  plain[6]=1;plain[7]=4;plain[8]=mtpc_boolTrue;for(int i=9;i<12;++i)plain[i]=i;
  if(variant==3)put64(2,3);
  if(variant==4)put64(4,(1700000000ULL<<32));
  if(variant==5)put64(4,(1700000000ULL<<32)|2);
  if(variant==6)plain[7]=1;
  if(variant==7)plain[7]=8;
  if(variant==8)plain[7]=-4;
  if(variant==9)put64(4,(1700000061ULL<<32)|1);
  if(variant==10)put64(4,(1699999699ULL<<32)|3);
  if(variant==11)put64(4,(1700000060ULL<<32)|1);
  if(variant==12)put64(4,(1699999700ULL<<32)|3);
  if(variant==13){plain.resize(268);plain[7]=4;}
  unsigned char hash[32];SHA256_CTX context;SHA256_Init(&context);SHA256_Update(&context,auth->partForMsgKey(false),32);SHA256_Update(&context,plain.data(),plain.size()*4);SHA256_Final(hash,&context);
  MTPint128 msg;std::memcpy(&msg,hash+8,16);MTPint256 key,iv;auth->prepareAES(msg,key,iv,false);
  mtpBuffer packet(6+plain.size());auto id=auth->keyId();std::memcpy(packet.data(),&id,8);std::memcpy(packet.data()+2,&msg,16);MTP::aesIgeEncryptRaw(plain.data(),packet.data()+6,plain.size()*4,&key,&iv);
  if(variant==1)packet[0]^=1;if(variant==2)packet[2]^=1;
  if(variant>=14&&variant<=16)for(int i=0;i<variant-13;++i)packet.push_back(0x1020304);
  if(variant==17)packet.resize(17);
  if(variant==18)packet.resize((16*1024*1024)/4+1);
  return packet;
 };
 for(int variant=0;variant<19;++variant){auto packet=make(variant);MTP::details::Connection connection;connection.queue.push_back(packet);MTP::details::SessionPrivate instance;instance._connection=&connection;instance._encryptionKey=auth;instance._keyId=auth->keyId();instance.handleReceived();
  uint32_t header[]={uint32_t(variant),uint32_t(variant==18?0:packet.size()),uint32_t(instance.accepted),uint32_t(instance.bad_time),uint32_t(instance.output.size())};std::fwrite(header,4,5,f);if(variant!=18)std::fwrite(packet.data(),4,packet.size(),f);std::fwrite(instance.output.data(),4,instance.output.size(),f);
 }
 std::fclose(f);
}
'''
out.write_text(prefix+compare+method+suffix)
