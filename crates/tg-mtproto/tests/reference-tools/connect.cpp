#include <QtCore/QCoreApplication>
#include <QtCore/QEventLoop>
#include <QtCore/QTimer>
#include <QtNetwork/QTcpServer>
#include <QtNetwork/QTcpSocket>
#include <QtNetwork/QNetworkProxy>
#include <cstdio>
int main(int argc,char**argv){QCoreApplication app(argc,argv);if(argc!=2)return 1;FILE*f=std::fopen(argv[1],"wb");if(!f)return 2;
for(bool auth:{false,true}){QTcpServer server;if(!server.listen(QHostAddress::LocalHost))return 3;QTcpSocket client;QEventLoop loop;int stage=0;QByteArray pending;client.setProxy(QNetworkProxy(QNetworkProxy::HttpProxy,"127.0.0.1",server.serverPort(),auth?QString("user"):QString(),auth?QString("pass"):QString()));
QObject::connect(&server,&QTcpServer::newConnection,[&]{auto socket=server.nextPendingConnection();QObject::connect(socket,&QTcpSocket::readyRead,[&,socket]{pending+=socket->readAll();if(!pending.endsWith("\r\n\r\n"))return;uint32_t n=pending.size();std::fwrite(&n,4,1,f);std::fwrite(pending.data(),1,n,f);pending.clear();if(auth&&stage==0){socket->write("HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=\"test\"\r\nContent-Length: 0\r\n\r\n");stage=1;}else{stage=2;loop.quit();}});});
client.connectToHost("example.com",443);QTimer::singleShot(2000,&loop,&QEventLoop::quit);loop.exec();if(stage!=2)return 4;client.abort();}
std::fclose(f);}
