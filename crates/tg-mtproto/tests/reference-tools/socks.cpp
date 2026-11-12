#include <QtCore/QCoreApplication>
#include <QtCore/QEventLoop>
#include <QtCore/QTimer>
#include <QtNetwork/QTcpServer>
#include <QtNetwork/QTcpSocket>
#include <QtNetwork/QNetworkProxy>
#include <cstdio>
int main(int argc,char**argv){QCoreApplication app(argc,argv);if(argc!=2)return 1;FILE*f=std::fopen(argv[1],"wb");if(!f)return 2;
for(bool auth:{false,true}){QTcpServer server;if(!server.listen(QHostAddress::LocalHost))return 3;QTcpSocket client;QEventLoop loop;int stage=0;client.setProxy(QNetworkProxy(QNetworkProxy::Socks5Proxy,"127.0.0.1",server.serverPort(),auth?QString("user"):QString(),auth?QString("pass"):QString()));
QObject::connect(&server,&QTcpServer::newConnection,[&]{auto socket=server.nextPendingConnection();QObject::connect(socket,&QTcpSocket::readyRead,[&,socket]{const auto bytes=socket->readAll();uint32_t n=bytes.size();std::fwrite(&n,4,1,f);std::fwrite(bytes.data(),1,n,f);if(stage==0){socket->write(auth?QByteArray::fromHex("0502"):QByteArray::fromHex("0500"));stage=auth?1:2;}else if(stage==1){socket->write(QByteArray::fromHex("0100"));stage=2;}else{stage=3;loop.quit();}});});
client.connectToHost("example.com",443);QTimer::singleShot(2000,&loop,&QEventLoop::quit);loop.exec();if(stage!=3)return 4;client.abort();}
std::fclose(f);}
