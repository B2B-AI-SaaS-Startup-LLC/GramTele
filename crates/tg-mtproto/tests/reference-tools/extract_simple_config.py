"""Execute the original decryptSimpleConfig with an independently generated signing key."""
from pathlib import Path
import sys
source=Path(sys.argv[1])/ 'SourceFiles/mtproto/special_config_request.cpp'
s=source.read_text();method=s[s.index('bool SpecialConfigRequest::decryptSimpleConfig('):s.index('\nvoid SpecialConfigRequest::handleResponse(')]
method=method.replace('SpecialConfigRequest::','Capture::').replace('kPublicKey','testPublicKey')
method=method.replace('return false;', 'std::fprintf(stderr, "native failure at %d\\n", __LINE__);return false;')
prefix=r'''
#include "mtproto/details/mtproto_rsa_public_key.h"
#include "mtproto/mtproto_auth_key.h"
#include "base/openssl_help.h"
#include <openssl/pem.h>
#include <openssl/sha.h>
#include <cstdio>
namespace Logs {bool DebugEnabled(){return false;}}
namespace MTP::details {
std::string testPublicKey;
struct Capture {MTPhelp_ConfigSimple _simpleConfig;bool decryptSimpleConfig(const QByteArray &bytes);};
'''
suffix=r'''
}
int main(int argc,char**argv){
 if(argc!=2)return 1;auto*f=std::fopen(argv[1],"wb");if(!f)return 2;
 auto*rsa=RSA_new();auto*e=BN_new();BN_set_word(e,65537);RSA_generate_key_ex(rsa,2048,e,nullptr);
 auto*bio=BIO_new(BIO_s_mem());PEM_write_bio_RSAPublicKey(bio,rsa);char*p=nullptr;auto n=BIO_get_mem_data(bio,&p);MTP::details::testPublicKey.assign(p,n);
 const uint32_t words[]={0x5a592a6c,1700000000,1700003600,1,0x4679b65f,0,2,1,0xd433ad73,0x959aa7a7,443};
 // Build TL with its actual original writer, independently of tg-tl-gen.
 MTPhelp_ConfigSimple config=MTP_help_configSimple(MTP_int(1700000000),MTP_int(1700003600),MTP_vector<MTPAccessPointRule>(1,MTP_accessPointRule(MTP_string(""),MTP_int(2),MTP_vector<MTPIpPort>(1,MTP_ipPort(MTP_int(0x959aa7a7),MTP_int(443))))));
 mtpBuffer tl;config.write(tl);unsigned char plain[256];for(int i=0;i<256;++i)plain[i]=i;plain[0]=1;auto length=uint32_t((tl.size()+1)*4);std::memcpy(plain+32,&length,4);std::memcpy(plain+36,tl.data(),tl.size()*4);unsigned char hash[32];SHA256(plain+32,208,hash);std::memcpy(plain+240,hash,16);unsigned char iv[16];std::memcpy(iv,plain+16,16);AES_KEY aes;AES_set_encrypt_key(plain,256,&aes);AES_cbc_encrypt(plain+32,plain+32,224,&aes,iv,AES_ENCRYPT);unsigned char encrypted[256];if(RSA_private_encrypt(256,plain,encrypted,rsa,RSA_NO_PADDING)!=256)return 3;auto encoded=QByteArray((const char*)encrypted,256).toBase64();MTP::details::Capture capture;if(!capture.decryptSimpleConfig(encoded))return 4;mtpBuffer result;capture._simpleConfig.write(result);if(result!=tl)return 5;
 auto write=[&](const void*data,uint32_t size){std::fwrite(&size,4,1,f);std::fwrite(data,1,size,f);};write(MTP::details::testPublicKey.data(),MTP::details::testPublicKey.size());write(encoded.data(),encoded.size());write(result.data(),result.size()*4);std::fclose(f);BIO_free(bio);RSA_free(rsa);BN_free(e);
}
'''
Path(sys.argv[2]).write_text(prefix+method+suffix)
