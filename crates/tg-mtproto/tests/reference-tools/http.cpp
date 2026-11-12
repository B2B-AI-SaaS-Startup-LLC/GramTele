// QNetworkAccessManager behavior used by original connection_http.cpp.
// Modes: sequential, concurrent, malformed (Telegram's double-space status).
#include <QtCore/QCoreApplication>
#include <QtCore/QEventLoop>
#include <QtCore/QTimer>
#include <QtNetwork/QNetworkAccessManager>
#include <QtNetwork/QNetworkRequest>
#include <QtNetwork/QNetworkProxy>
#include <QtNetwork/QNetworkReply>
#include <QtNetwork/QTcpServer>
#include <QtNetwork/QTcpSocket>
#include <cstdio>
#include <functional>
#include <memory>
int main(int argc,char **argv) {
 QCoreApplication app(argc,argv);
 if(argc!=3)return 1;
 const auto mode=QByteArray(argv[2]);
 const bool sequential=mode=="sequential",malformed=mode=="malformed";
 const int total=sequential?2:8;
 FILE *file=std::fopen(argv[1],"wb");if(!file)return 2;
 QTcpServer server;if(!server.listen(QHostAddress::LocalHost))return 3;
 QNetworkAccessManager manager;manager.setProxy(QNetworkProxy::NoProxy);
 QEventLoop loop;int connections=0,finished=0,requests=0,errors=0;
 QObject::connect(&server,&QTcpServer::newConnection,[&]{
  ++connections;auto socket=server.nextPendingConnection();
  auto pending=std::make_shared<QByteArray>();
  QObject::connect(socket,&QTcpSocket::readyRead,[&,socket,pending]{
   *pending+=socket->readAll();while(true){
    auto end=pending->indexOf("\r\n\r\n");if(end<0)return;
    auto header=pending->left(end+4);
    auto length=header.toLower().indexOf("content-length:");
    auto size=header.mid(length+15).split('\r').front().trimmed().toInt();
    if(pending->size()<end+4+size)return;
    auto bytes=pending->left(end+4+size);pending->remove(0,bytes.size());
    bytes.replace(QByteArray("127.0.0.1:")+QByteArray::number(server.serverPort()),"example.com");
    uint32_t n=bytes.size();std::fwrite(&n,4,1,file);std::fwrite(bytes.data(),1,n,file);++requests;
    QTimer::singleShot(100,socket,[socket,malformed]{
     socket->write(malformed?"HTTP/1.0  200 OK\r\nConnection: keep-alive\r\nContent-Length: 8\r\n\r\n12345678":"HTTP/1.0 200 OK\r\nConnection: keep-alive\r\nContent-Length: 8\r\n\r\n12345678");
    });
   }
  });
 });
 std::function<void()> send=[&]{
  QNetworkRequest request(QUrl(QString("http://127.0.0.1:%1/api").arg(server.serverPort())));
  request.setHeader(QNetworkRequest::ContentLengthHeader,12);
  request.setHeader(QNetworkRequest::ContentTypeHeader,"application/x-www-form-urlencoded");
  auto reply=manager.post(request,QByteArray::fromHex("010000000200000003000000"));
  QObject::connect(reply,&QNetworkReply::finished,[&,reply]{
   if(reply->error()!=QNetworkReply::NoError){++errors;std::fprintf(stderr,"Qt error: %s\n",qPrintable(reply->errorString()));}
   reply->readAll();reply->deleteLater();
   if(++finished==total)loop.quit();else if(sequential)send();
  });
 };
 if(sequential)send();else for(int i=0;i<total;++i)send();
 QTimer::singleShot(3000,&loop,&QEventLoop::quit);loop.exec();
 std::fprintf(stderr,"mode=%s connections=%d requests=%d finished=%d errors=%d\n",argv[2],connections,requests,finished,errors);
 std::fclose(file);return finished==total?0:4;
}
