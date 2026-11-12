"""Native original hello generator, with deterministic randomness/time hooks only."""
from pathlib import Path
import sys
root=Path(sys.argv[1]);out=Path(sys.argv[2]);s=(root/'SourceFiles/mtproto/details/mtproto_tls_socket.cpp').read_text();s=s[:s.index('TlsSocket::TlsSocket(')]
# rng-independent deterministic permutation is a valid fixed outcome of shuffle.
s=s.replace('ranges::shuffle(list);','std::reverse(list.begin(), list.end()); // fixed permutation entropy')
s=s.replace('EVP_PKEY_keygen(context, &key)','ReferenceKeygen(context, &key)')
# Original anonymous-namespace functions stay in this translation unit.
s+='''
void CaptureHello(FILE*f) {
 const auto rules=PrepareClientHelloRules();auto secret=bytes::array<16>();for(unsigned i=0;i<16;++i)secret[i]=bytes::type(i);
 const auto hello=PrepareClientHello(rules,bytes::make_span("example.com",11),secret);
 if(hello.data.isEmpty())std::abort();
 auto key=GeneratePublicKey();std::fwrite(key.data(),1,key.size(),f);
 uint32_t size=hello.data.size();std::fwrite(&size,4,1,f);std::fwrite(hello.data.data(),1,size,f);std::fwrite(hello.digest.data(),1,hello.digest.size(),f);
}
} // namespace MTP::details
int main(int argc,char**argv){if(argc!=2)return 1;FILE*f=std::fopen(argv[1],"wb");if(!f)return 2;MTP::details::CaptureHello(f);std::fclose(f);}
'''
prefix='''#include <openssl/evp.h>
#include <cstdio>
static unsigned entropy_position=1;
extern "C" int __wrap_RAND_bytes(unsigned char *data,int size){for(int i=0;i<size;++i)data[i]=entropy_position++;return 1;}
int ReferenceKeygen(EVP_PKEY_CTX*,EVP_PKEY** key){unsigned char secret[32];for(unsigned i=0;i<32;++i)secret[i]=i+1;*key=EVP_PKEY_new_raw_private_key(EVP_PKEY_X25519,nullptr,secret,32);return *key?1:0;}
namespace base::unixtime {int http_now(){return 1700000000;}}
'''
out.write_text(prefix+s)
