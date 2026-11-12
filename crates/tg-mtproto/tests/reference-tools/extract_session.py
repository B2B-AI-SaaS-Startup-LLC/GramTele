"""Execute original sendSecureRequest with only QObject/network sinks replaced."""
import pathlib,sys
root=pathlib.Path(sys.argv[1]);out=pathlib.Path(sys.argv[2])
source=(root/'SourceFiles/mtproto/session_private.cpp').read_text()
method=source[source.index('bool SessionPrivate::sendSecureRequest('):source.index('\nmtpRequestId SessionPrivate::wasSent')]
abstract=(root/'SourceFiles/mtproto/connection_abstract.cpp').read_text()
prepare=abstract[abstract.index('mtpBuffer AbstractConnection::prepareSecurePacket('):abstract.index('\ngsl::span<const mtpPrime> AbstractConnection::parseNotSecureResponse')]
prefix=r'''
#include "mtproto/mtproto_auth_key.h"
#include "mtproto/details/mtproto_serialized_request.h"
#include <openssl/sha.h>
#include <cstdio>
#undef MTP_LOG
#define MTP_LOG(...) do {} while(false)
namespace crl {time now(){return 0;}}
static unsigned counter=0;
extern "C" int __wrap_RAND_bytes(unsigned char* data,int size){for(int i=0;i<size;++i)data[i]=counter++;return 1;}
namespace MTP::details {
class AbstractConnection {public:
 mtpBuffer packet;
 mtpBuffer prepareSecurePacket(uint64,MTPint128,uint32)const;
 void setSentEncryptedWithKeyId(uint64){}
 void sendData(mtpBuffer&& value){packet=std::move(value);}
};
class SessionPrivate {public:
 uint64 _sessionSalt=0x1122334455667788ULL,_sessionId=0x8877665544332211ULL,_keyId=0;
 AuthKeyPtr _encryptionKey;AbstractConnection *_connection=nullptr;
 bool sendSecureRequest(SerializedRequest&&,bool);void onSentSome(uint64){}
};
'''
suffix=r'''
}
int main(int argc,char**argv){
 if(argc!=2)return 1;FILE*f=std::fopen(argv[1],"wb");if(!f)return 2;
 MTP::AuthKey::Data data;for(unsigned i=0;i<256;++i)data[i]=gsl::byte(i);
 MTP::details::AbstractConnection connection;MTP::details::SessionPrivate session;
 session._encryptionKey=std::make_shared<MTP::AuthKey>(data);session._keyId=session._encryptionKey->keyId();session._connection=&connection;
 auto request=MTP::details::SerializedRequest::Serialize(MTPhelp_GetConfig());
 request.setMsgId((1700000000ULL<<32)|4);request.setSeqNo(1);
 if(!session.sendSecureRequest(std::move(request),false))return 3;
 uint32_t n=connection.packet.size()*4;std::fwrite(&n,4,1,f);std::fwrite(connection.packet.data(),1,n,f);
 // Separate incoming sample using original receive-direction AuthKey schedule.
 auto plain=*request;plain[4]=1;
 unsigned char hash[32];SHA256_CTX context;SHA256_Init(&context);
 SHA256_Update(&context,session._encryptionKey->partForMsgKey(false),32);
 SHA256_Update(&context,plain.data(),plain.size()*4);SHA256_Final(hash,&context);
 MTPint128 msg;std::memcpy(&msg,hash+8,16);MTPint256 key,iv;
 session._encryptionKey->prepareAES(msg,key,iv,false);
 auto packet=connection.prepareSecurePacket(session._keyId,msg,plain.size());auto prefix=packet.size();packet.resize(prefix+plain.size());
 MTP::aesIgeEncryptRaw(plain.data(),packet.data()+prefix,plain.size()*4,&key,&iv);
 n=packet.size()*4;std::fwrite(&n,4,1,f);std::fwrite(packet.data(),1,n,f);std::fclose(f);
}
'''
out.write_text(prefix+prepare+method+suffix)
