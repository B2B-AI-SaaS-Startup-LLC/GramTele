"""Extract original protocol and prefix code, changing only harness names.
No Rust code or port parser participates in the C++ expected bytes.
"""
from pathlib import Path
import sys
root=Path(sys.argv[1]);out=Path(sys.argv[2]);source=(root/'SourceFiles/mtproto/connection_tcp.cpp').read_text();socket=(root/'SourceFiles/mtproto/details/mtproto_tcp_socket.cpp').read_text()
protocol=source[source.index('class TcpConnection::Protocol {'):source.index('TcpConnection::TcpConnection(')]
body=source[source.index('bytes::const_span TcpConnection::prepareConnectionStartPrefix('):source.index('void TcpConnection::disconnectFromServer()')]
body=body.replace('TcpConnection::prepareConnectionStartPrefix','ReferenceConnection::prepareConnectionStartPrefix')
check=socket[socket.index('bool TcpSocket::isGoodStartNonce('):socket.index('void TcpSocket::timedOut()')].replace('bool TcpSocket::isGoodStartNonce(bytes::const_span nonce)','bool GoodNonce(bytes::const_span nonce)')
s='''#include "scheme.h"
#include "mtproto/mtproto_auth_key.h"
#include "base/openssl_help.h"
#include "base/random.h"
#include <range/v3/all.hpp>
#include <cstdio>
static unsigned entropy_position=1;
extern "C" int __wrap_RAND_bytes(unsigned char* data,int size){for(int i=0;i<size;++i)data[i]=entropy_position++;return 1;}
namespace MTP::details {
constexpr int kPacketSizeMax=0x1000000*4,kConnectionStartPrefixSize=64;
class TcpConnection {public:class Protocol;};
'''+protocol+check+'''
struct ReferenceSocket {bool isGoodStartNonce(bytes::const_span n){return GoodNonce(n);}};
struct ReferenceConnection {
 bool _connectionStarted=false;ReferenceSocket socket;ReferenceSocket *_socket=&socket;
 std::unique_ptr<TcpConnection::Protocol> _protocol;
 uchar _sendKey[32],_receiveKey[32];CTRState _sendState,_receiveState;int16 _protocolDcId=10002;
 bytes::const_span prepareConnectionStartPrefix(bytes::span buffer);
};
'''+body+'''
void Capture(FILE*f,int version) {
 entropy_position=1;ReferenceConnection connection;
 auto secret=bytes::vector();if(version==1)secret.resize(16,bytes::type(0x11));if(version==2){secret.resize(17,bytes::type(0x11));secret[0]=bytes::type(0xdd);}
 connection._protocol=TcpConnection::Protocol::Create(secret);
 auto header=bytes::array<64>();connection.prepareConnectionStartPrefix(header);
 auto buffer=mtpBuffer{0,0,0x12345678,-1,0x07060504};auto packet=connection._protocol->finalizePacket(buffer);aesCtrEncrypt(packet,connection._sendKey,&connection._sendState);
 uint32_t size=64+packet.size();std::fwrite(&size,4,1,f);std::fwrite(header.data(),1,64,f);std::fwrite(packet.data(),1,packet.size(),f);
}
}
int main(int argc,char**argv){if(argc!=2)return 1;FILE*f=std::fopen(argv[1],"wb");if(!f)return 2;for(int v=0;v<3;++v)MTP::details::Capture(f,v);std::fclose(f);}
'''
out.write_text(s)
