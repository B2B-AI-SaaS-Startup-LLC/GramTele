#include "storage/cache/storage_cache_database.h"
#include "storage/storage_encryption.h"
#include "storage/storage_encrypted_file.h"
#include "base/concurrent_timer.h"
#include <QtCore/QCoreApplication>
#include <QtCore/QThreadPool>
#include <crl/crl_on_main.h>
#include <rpl/rpl.h>
#include <chrono>
#include <sys/resource.h>
#include <csignal>

std::atomic<unsigned> RandomCounter{0};
std::atomic<bool> WideRandom{false};
std::atomic<std::time_t> Wall{1700000000};
extern "C" int __wrap_RAND_bytes(unsigned char *data,int count) {
 for (auto i=0;i<count;++i) {auto n=RandomCounter.fetch_add(1);data[i]=(WideRandom?(n^(n>>8)^(n>>16)):n)&255;}
 return 1;
}
extern "C" std::time_t __wrap_time(std::time_t *out) {auto t=Wall.load();if(out)*out=t;return t;}
using DB=Storage::Cache::Database;
using Key=Storage::Cache::Key;
auto key() {auto b=bytes::vector(64);for(auto i=0;i<64;++i)b[i]=std::byte(i);return Storage::EncryptionKey(std::move(b));}
auto id(int i) {return Key{0x123456789abcdef0ULL,uint64(i+1)};}
auto value(int i,int n=21) {auto b=QByteArray(n,Qt::Uninitialized);for(auto j=0;j<n;++j)b[j]=char(i*31+j);return b;}
void check(bool good,const char *message){if(!good){std::cerr<<message<<"\n";std::exit(4);}}
void pump(DB &db) {db.sync();QCoreApplication::processEvents();}
template<typename Predicate>void until(DB &db,Predicate predicate) {
 const auto end=std::chrono::steady_clock::now()+std::chrono::seconds(20);
 while(!predicate()){pump(db);check(std::chrono::steady_clock::now()<end,"queue/timer deadline");}
}
void put(DB &db,int i,int v,int n=21) {auto good=false;db.put(id(i),DB::TaggedValue(value(v,n),uint8(i+1)),[&](auto e){good=e.type==Storage::Cache::Error::Type::None;});db.sync();check(good,"put");}
void get(DB &db,int i,int v,int n=21) {auto good=false;db.getWithTag(id(i),[&](auto b){good=b.bytes==value(v,n)&&b.tag==uint8(i+1);});db.sync();check(good,"get");}
bool compacted(const QString &base) {
 Storage::File f;if(f.open(base+"/0/binlog",Storage::File::Mode::Read,key())!=Storage::File::Result::Success)return false;
 if(!f.seek(16))return false;auto b=bytes::vector(16);return f.read(b)==16&&b[0]==std::byte(2)&&!QFile::exists(base+"/0/binlog-temp")&&!QFile::exists(base+"/0/binlog-ready");
}
DB::Settings settings(const QString &name) {
 auto s=DB::Settings();s.maxDataSize=127;s.pruneTimeout=0;
 if(name=="plain")s.trackEstimatedTime=false;
 if(name=="size"||name=="size-ties"||name=="time"){s.totalTimeLimit=0;s.writeBundleDelay=0;s.totalSizeLimit=(name=="size"||name=="size-ties")?200:0;}
 if(name=="compact"||name=="all-records")s.compactAfterExcess=48;
 if(name=="wide"){s.maxDataSize=(1<<24)-1;s.maxBundledRecords=257;s.compactAfterExcess=48;}
 return s;
}
void scenario(const QString &root,const QString &name,bool generate) {
 WideRandom=(name=="wide");RandomCounter=0;Wall=1700000000;const auto base=root+'/'+name;auto s=settings(name);if(!generate)s.compactAfterExcess=0;if(generate&&(name=="size"||name=="size-ties"))s.totalSizeLimit=0;
 DB db(base,s);Storage::Cache::Error error;
 db.open(key(),[&](auto e){error=e;});db.sync();check(error.type==Storage::Cache::Error::Type::None,"open");
 auto stats=DB::Stats();auto got=false;auto life=rpl::lifetime();
 db.statsOnMain()|rpl::on_next([&](auto v){stats=std::move(v);got=true;},life);
 until(db,[&]{return got;});
 if(generate){
  if(name=="empty"){}
  else if(name=="wide"){for(auto i=0;i<257;++i)put(db,i,i,i==0?0x12345:21);put(db,0,9,0x12345);until(db,[&]{return compacted(base);});}
  else if(name=="size"||name=="size-ties"||name=="time"){
   for(auto i=0;i<3;++i){Wall=1700000000+(name=="size-ties"?0:i*3);put(db,i,i,120);}
   if(name=="time"){Wall=1700000010;auto u=DB::SettingsUpdate();u.totalSizeLimit=0;u.totalTimeLimit=5;db.updateSettings(u);}else{auto u=DB::SettingsUpdate();u.totalSizeLimit=200;u.totalTimeLimit=0;db.updateSettings(u);}
   until(db,[&]{return stats.full.count==1;});
  }else{
   for(auto i=0;i<4;++i)put(db,i,i);
   if(name=="removed"){for(auto i=0;i<4;++i)db.remove(id(i));}
   else if(name=="compact"||name=="all-records"){
    put(db,0,9);until(db,[&]{return compacted(base);});
    if(name=="all-records"){put(db,4,4);db.remove(id(1));Wall=1700000002;get(db,2,2);}
   }else{db.remove(id(1));Wall=1700000002;get(db,0,0);}
  }
 }else{
  if(name=="empty"||name=="removed")check(stats.full.count==0,"empty count");
  else if(name=="wide"){check(stats.full.count==257,"wide count");for(auto i=0;i<257;++i)get(db,i,i==0?9:i,i==0?0x12345:21);}
  else if(name=="size"||name=="size-ties"||name=="time"){check(stats.full.count==1,"evicted count");get(db,name=="size-ties"?0:2,name=="size-ties"?0:2,120);}
  else if(name=="compact"){check(stats.full.count==4,"compact count");get(db,0,9);for(auto i=1;i<4;++i)get(db,i,i);}
  else if(name=="all-records"){check(stats.full.count==4,"all count");get(db,0,9);for(auto i=2;i<5;++i)get(db,i,i);}
  else{check(stats.full.count==3,"store count");get(db,0,0);get(db,2,2);get(db,3,3);}
 }
 auto cleaned=false;db.waitForCleaner([&]{cleaned=true;});until(db,[&]{return cleaned;});
 db.close();db.sync();life.destroy();QCoreApplication::processEvents();
 std::cout<<name.toStdString()<<" "<<(generate?"generate":"verify")<<" PASS\n";
}
#include "probes.hpp"
#include "cleaner-check.hpp"
void faults(const QString &root) {
 check(!QFile::exists(root),"new fault directory required");check(QDir().mkpath(root),"fault mkdir");
 for (auto large:{false,true}) {
  struct rlimit old;check(getrlimit(RLIMIT_FSIZE,&old)==0,"getrlimit");
  auto limit=old;limit.rlim_cur=124;auto handler=signal(SIGXFSZ,SIG_IGN);check(setrlimit(RLIMIT_FSIZE,&limit)==0,"setrlimit");
  Storage::File f;const auto path=root+(large?"/large":"/small");const auto opened=f.open(path,Storage::File::Mode::Write,key());
  auto bytes=bytes::vector(large?32768:16,std::byte(7));const auto original=bytes;
  const auto wrote=f.write(bytes);const auto flushed=f.flush();
  std::cout<<(large?"large":"small")<<','<<int(opened)<<','<<wrote<<','<<flushed<<','<<f.offset()<<','<<f.size()<<','<<(bytes==original)<<','<<QFileInfo(path).size()<<'\n';
  f.close();check(setrlimit(RLIMIT_FSIZE,&old)==0,"restore rlimit");signal(SIGXFSZ,handler);
 }
 Storage::File f;auto opened=f.open("/dev/full",Storage::File::Mode::Write,key());auto b=bytes::vector(16,std::byte(7));auto wrote=f.write(b);auto flushed=f.flush();
 std::cout<<"full,"<<int(opened)<<','<<wrote<<','<<flushed<<','<<f.offset()<<','<<f.size()<<'\n';
}
int main(int argc,char **argv) {
 QCoreApplication app(argc,argv);
 crl::init_main_queue([](void(*method)(void*),void *argument){QMetaObject::invokeMethod(QCoreApplication::instance(),[=]{method(argument);},Qt::QueuedConnection);});
 base::ConcurrentTimerEnvironment timers;
 if(argc!=3)return 2;if(QString::fromUtf8(argv[1])=="cleaner"){cleaner_check(QString::fromUtf8(argv[2]));return 0;}if(QString::fromUtf8(argv[1])=="faults"){faults(QString::fromUtf8(argv[2]));return 0;}if(QString::fromUtf8(argv[1])=="probes"){probes(QString::fromUtf8(argv[2]));return 0;}const auto generate=QString::fromUtf8(argv[1])=="generate";
 const auto root=QDir(QString::fromUtf8(argv[2])).absolutePath();
 if(generate){check(!QFile::exists(root),"generation requires a new synthetic directory");check(QDir().mkpath(root),"mkdir");}
 for(const auto &name:{"empty","stores","plain","removed","size","size-ties","time","compact","all-records","wide"})scenario(root,QString::fromUtf8(name),generate);
 QThreadPool::globalInstance()->waitForDone();QCoreApplication::processEvents();
}
