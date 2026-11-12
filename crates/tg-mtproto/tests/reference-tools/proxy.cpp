#include "core/core_settings_proxy.cpp"
#include <cstdio>
namespace Core {
void CaptureProxy(FILE*f){for(int variant=0;variant<3;++variant){auto proxy=MTP::ProxyData();if(variant>0){proxy.type=MTP::ProxyData::Type::Mtproto;proxy.host="proxy.example.com";proxy.port=443;proxy.password="000102030405060708090a0b0c0d0e0f";}if(variant==2)proxy.user=QString("");const auto bytes=SerializeProxyData(proxy);uint32_t n=bytes.size();std::fwrite(&n,4,1,f);std::fwrite(bytes.data(),1,n,f);}}
}
int main(int argc,char**argv){if(argc!=2)return 1;FILE*f=std::fopen(argv[1],"wb");if(!f)return 2;Core::CaptureProxy(f);std::fclose(f);}
