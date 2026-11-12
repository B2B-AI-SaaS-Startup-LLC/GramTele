// Owner/OS harness only: format methods are extracted verbatim from the original.
#include "utilities.h"
#include "base/random.h"
#include "storage/serialize_common.h"
#include <QtCore/QDir>
#include <QtCore/QFile>
#include <cstdio>
extern void reset_entropy();
QString captureRoot;
namespace crl {time now(){return 0;}}
namespace MTP {
class Config {public:bool isTestMode()const{return false;}};
struct NeverInstance {int mainDcId()const{return 2;}AuthKeysList getKeysForWrite()const{return {};}};
}
struct CaptureUserId {uint64 bare=0;};
using UserId=CaptureUserId;
namespace Main {
class Domain;
class Account {
public:
 Account(Domain*,const QString &dataName,int index);
 QByteArray serializeMtpAuthorization()const;
 void setMtpAuthorization(const QByteArray&);
 void setSessionUserId(uint64 v){_sessionUserId.bare=v;}
 bool sessionExists()const{return _sessionUserId.bare!=0;}
 const Account &session()const{return *this;}
 CaptureUserId userId()const{return _sessionUserId;}
 std::unique_ptr<MTP::Config> prepareToStart(MTP::AuthKeyPtr key);
 uint64 willHaveSessionUniqueId(MTP::Config*)const{return _sessionUserId.bare;}
 void start(std::unique_ptr<MTP::Config>){}
 CaptureUserId _sessionUserId;
 struct Fields {int mainDcId=0;MTP::AuthKeysList keys;} _mtpFields;
 MTP::AuthKeysList _mtpKeysToDestroy;
 MTP::NeverInstance *_mtp=nullptr,*_mtpForKeysDestroy=nullptr;
 QString dataName;
};
class Domain {
public:
 static constexpr int kPremiumMaxAccounts=200;
 struct AccountWithIndex{int index=0;std::unique_ptr<Account> account;};
 std::vector<AccountWithIndex> list;
 int active=0;
 void accountAddedInStorage(AccountWithIndex a){list.push_back(std::move(a));}
 void activateFromStorage(int i){active=i;}
 const auto&accounts()const{return list;}
 int activeForStorage()const{return active;}
};
constexpr auto kWideIdsTag=~uint64(0);
#include "authorization.inc"
QString compose(QString name,int index){name.replace('#',QString());if(index>0)name+='#'+QString::number(index+1);return name;}
Account::Account(Domain*,const QString&name,int index):dataName(compose(name,index)){}
std::unique_ptr<MTP::Config> Account::prepareToStart(MTP::AuthKeyPtr key){
 using namespace Storage::details;
 auto hash=uint64();auto md5=HashMd5(dataName.toUtf8().constData(),dataName.toUtf8().size());memcpy(&hash,md5.result(),8);
 auto base=captureRoot+ToFilePart(hash)+'/';FileReadDescriptor map;
 // No settings/cache objects: validate the original map envelope and read the original authorization payload.
 if(!ReadFile(map,u"map"_q,base))return nullptr;QByteArray a,b,c;map.stream>>a>>b>>c;EncryptedDescriptor plain;if(!DecryptLocal(plain,c,key))return nullptr;
 FileReadDescriptor mtp;if(!ReadEncryptedFile(mtp,ToFilePart(hash),captureRoot,key))return nullptr;quint32 block=0;QByteArray auth;mtp.stream>>block>>auth;if(block!=0x4b)std::abort();setMtpAuthorization(auth);return nullptr;
}
}
// Preserve the original storage Domain layout and declarations, exposing state only to fixture setup.
#include "domain_header.h"
namespace Storage {
using namespace details;
constexpr int AppVersion=7002009,LocalEncryptSaltSize=32;
QString BaseGlobalPath(){return captureRoot;}
QString ComputeKeyName(const QString&name){return "key_"+name;}
Domain::Domain(not_null<Main::Domain*> owner,const QString&name):_owner(owner),_dataName(name){}
Domain::~Domain()=default;
#include "domain.inc"
}
MTP::AuthKeyPtr key(int dc,int account,int slot){MTP::AuthKey::Data data;for(int i=0;i<256;++i)data[i]=gsl::byte((i+account*17+slot*31)&255);return std::make_shared<MTP::AuthKey>(MTP::AuthKey::Type::ReadFromFile,dc,data);}
QByteArray expected(int account){Main::Domain owner;Main::Account a(&owner,"data",account);a.setSessionUserId(1234567890123ULL+account);a._mtpFields.mainDcId=2;a._mtpFields.keys={key(2,account,0),key(4,account,1)};a._mtpKeysToDestroy={key(1,account,2)};return a.serializeMtpAuthorization();}
void create(const QString&dir,bool pass,int count){
 captureRoot=dir+'/';QDir().mkpath(captureRoot);reset_entropy();Main::Domain owner;Storage::Domain domain(&owner,"data");MTP::AuthKey::Data local;for(int i=0;i<256;++i)local[i]=gsl::byte(i);domain._localKey=std::make_shared<MTP::AuthKey>(local);domain.encryptLocalKey(pass?QByteArray("test-passcode"):QByteArray());
 for(int i=0;i<count;++i){auto account=std::make_unique<Main::Account>(&owner,"data",i);account->setMtpAuthorization(expected(i));auto hash=uint64();auto bytes=account->dataName.toUtf8();memcpy(&hash,HashMd5(bytes.data(),bytes.size()).result(),8);auto part=Storage::details::ToFilePart(hash);
  {Storage::details::FileWriteDescriptor file(u"map"_q,captureRoot+part+'/');file.writeData(QByteArray());file.writeData(QByteArray());Storage::details::EncryptedDescriptor data(0);file.writeEncrypted(data,domain._localKey);}
  {Storage::details::FileWriteDescriptor file(part,captureRoot);Storage::details::EncryptedDescriptor data(0);data.stream<<quint32(0x4b)<<expected(i);file.writeEncrypted(data,domain._localKey);}
  owner.accountAddedInStorage({i,std::move(account)});
 }
 owner.active=count-1;domain.writeAccounts();
}
bool verify(const QString&dir,bool pass,int count){captureRoot=dir+'/';Main::Domain owner;Storage::Domain domain(&owner,"data");if(domain.startModern(pass?QByteArray("test-passcode"):QByteArray())!=Storage::Domain::StartModernResult::Success)return false;if(owner.list.size()!=count||owner.active!=count-1)return false;for(int i=0;i<count;++i)if(owner.list[i].account->serializeMtpAuthorization()!=expected(i))return false;return true;}
int main(int argc,char**argv){if(argc!=3)return 1;
if(QByteArray(argv[1])=="samples"){
 QFile file(QString::fromUtf8(argv[2]));if(!file.open(QIODevice::WriteOnly))return 3;QDataStream stream(&file);stream.setVersion(QDataStream::Qt_5_1);
 const char16_t raw[]={0xd800,0x41,0xdc00};stream<<QByteArray()<<QByteArray("")<<QString()<<QString("")<<QString::fromUtf16(raw,3);Serialize::writeColor(stream,QColor(1,2,3,4));
 auto salt=QByteArray(32,Qt::Uninitialized);for(int i=0;i<32;++i)salt[i]=i;
 for(auto pass:{QByteArray(),QByteArray("legacy-pass")}){auto key=Storage::details::CreateLegacyLocalKey(pass,salt);key->write(stream);}
 for(auto pass:{QByteArray(),QByteArray("pass")}){auto key=Storage::details::CreateLocalKey(pass,salt);key->write(stream);}
 Main::Domain owner;Main::Account legacyAccount(&owner,"data",0);auto modern=expected(0);QByteArray old;{QDataStream stream(&old,QIODevice::WriteOnly);stream.setVersion(QDataStream::Qt_5_1);stream<<qint32(12345)<<qint32(2);}old+=modern.mid(20);legacyAccount.setMtpAuthorization(old);stream<<old<<legacyAccount.serializeMtpAuthorization();
 return 0;
}
const auto root=QString::fromUtf8(argv[2]);const bool write=QByteArray(argv[1])=="create";for(auto pass:{false,true})for(auto count:{1,3}){auto name=QString(pass?"pass-":"nopass-")+QString::number(count);auto path=root+'/'+name;if(write)create(path,pass,count);if(!verify(path,pass,count)){std::fprintf(stderr,"FAIL %s\n",qPrintable(name));return 2;}std::printf("%s: %s, accounts=%d, all keys and active index match\n",write?"original write/read":"original read Rust",qPrintable(name),count);}return 0;}
