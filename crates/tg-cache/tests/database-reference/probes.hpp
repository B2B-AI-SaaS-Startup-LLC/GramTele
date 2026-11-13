// Original-code recovery oracle. The mutation harness only touches synthetic
// copies and never substitutes database parsing, pruning or I/O algorithms.
QString place_name(const Storage::Cache::details::PlaceId &place) {
 QString s;const auto hex=QByteArray("0123456789ABCDEF");for(auto i=0;i<7;++i){if(i==1)s+='/';s+=hex[place[i]&15];s+=hex[place[i]>>4];}return s;
}
void copy_tree(const QString &from,const QString &to) {
 check(QDir().mkpath(to),"copy mkdir");for(const auto &entry:QDir(from).entryInfoList(QDir::Files|QDir::Dirs|QDir::NoDotAndDotDot)){
  if(entry.isDir())copy_tree(entry.absoluteFilePath(),to+'/'+entry.fileName());else check(QFile::copy(entry.absoluteFilePath(),to+'/'+entry.fileName()),"copy file");
 }
}
QByteArray binlog_plain(const QString &base) {
 Storage::File f;check(f.open(base+"/0/binlog",Storage::File::Mode::Read,key())==Storage::File::Result::Success,"probe read");auto bytes=QByteArray(f.size(),Qt::Uninitialized);check(f.read(bytes::make_detached_span(bytes))==bytes.size(),"probe plain");return bytes;
}
void rewrite_binlog(const QString &base,QByteArray b) {
 Storage::File f;check(f.open(base+"/0/binlog",Storage::File::Mode::Write,key())==Storage::File::Result::Success,"probe rewrite");check(f.write(bytes::make_detached_span(b))&&f.flush(),"probe write");
}
const char *const ProbeNames[]={"torn-tail","unknown-tail","zero-store","zero-remove","large-count","zero-size","large-size","multi-partial","missing-value","short-value","short-value-header","checksum","wrong-value-key","wrong-binlog-key","short-binlog","bad-format","track-mismatch","wrong-key-clear","bad-version","negative-version","ready-recovery","pending-access-prune"};
void probes(const QString &root) {
 check(!QFile::exists(root),"new probes directory required");check(QDir().mkpath(root),"probe mkdir");
 const auto fixtures=QString::fromUtf8(DATABASE_FIXTURES)+"/stores";
 QFile report(root+"/results.csv");check(report.open(QIODevice::WriteOnly),"probe report");
 for(auto name:ProbeNames){
  const auto kind=QString::fromUtf8(name);const auto base=root+'/'+kind;copy_tree(fixtures,base);auto plain=binlog_plain(base);auto record=Storage::Cache::details::StoreWithTime();memcpy(&record,plain.constData()+16,sizeof(record));const auto path=base+"/0/"+place_name(record.place);
  if(kind=="torn-tail"){QFile f(base+"/0/binlog");check(f.open(QIODevice::ReadWrite)&&f.resize(f.size()-5),"truncate tail");}
  else if(kind=="unknown-tail"||kind=="zero-store"||kind=="zero-remove"||kind=="large-count"||kind=="zero-size"||kind=="large-size"||kind=="multi-partial"){
   plain.truncate(16+4*48);
   const auto append=[&](const auto &v){plain+=QByteArray::fromRawData(reinterpret_cast<const char*>(&v),sizeof(v));};
   if(kind=="unknown-tail")plain+=QByteArray(16,char(255));
   else if(kind=="zero-store")append(Storage::Cache::details::MultiStoreWithTime(0));
   else if(kind=="zero-remove")append(Storage::Cache::details::MultiRemove(0));
   else if(kind=="large-count")append(Storage::Cache::details::MultiRemove(16385));
   else if(kind=="zero-size"){record.setSize(0);append(record);}
   else if(kind=="large-size"){record.setSize(128);append(record);}
   else{append(Storage::Cache::details::MultiStoreWithTime(2));record.key=id(9);append(record);record.key=id(10);record.setSize(0);append(record);}
   rewrite_binlog(base,plain);
  }else if(kind=="missing-value")check(QFile::remove(path),"remove value");
  else if(kind=="short-value"||kind=="short-value-header"){QFile f(path);check(f.open(QIODevice::ReadWrite)&&f.resize(kind=="short-value"?124:110),"truncate value");}
  else if(kind=="checksum"||kind=="wrong-value-key"){QFile f(path);check(f.open(QIODevice::ReadWrite),"mutate value");auto b=f.readAll();b[kind=="checksum"?116:4]^=1;check(f.seek(0)&&f.write(b)==b.size(),"mutation");}
  else if(kind=="short-binlog"){QFile f(base+"/0/binlog");check(f.open(QIODevice::WriteOnly)&&f.write("bad",3)==3,"bad binlog");}
  else if(kind=="bad-format"||kind=="track-mismatch"){plain[kind=="bad-format"?0:1]=kind=="bad-format"?1:0;rewrite_binlog(base,plain);}
  else if(kind=="bad-version"){QFile f(base+"/version");check(f.open(QIODevice::WriteOnly)&&f.write("bad",3)==3,"bad version");}
  else if(kind=="negative-version")check(Storage::Cache::details::WriteVersionValue(base+'/',-1),"negative version");
  else if(kind=="ready-recovery"){check(QFile::copy(base+"/0/binlog",base+"/0/binlog-ready"),"ready");QFile f(base+"/0/binlog");check(f.open(QIODevice::WriteOnly)&&f.write("bad",3)==3,"bad original");}
  RandomCounter=0;Wall=(kind=="pending-access-prune"?1700000005:1700000010);auto s=settings("stores");s.clearOnWrongKey=(kind=="wrong-key-clear");DB db(base,s);auto encryption=key();if(kind=="wrong-binlog-key"||kind=="wrong-key-clear"){encryption=Storage::EncryptionKey(bytes::vector(64,std::byte(99)));}
  auto error=Storage::Cache::Error();db.open(std::move(encryption),[&](auto e){error=e;});db.sync();
  auto got=false;auto stats=DB::Stats();auto life=rpl::lifetime();db.statsOnMain()|rpl::on_next([&](auto v){stats=std::move(v);got=true;},life);until(db,[&]{return got;});auto before=stats.full.count;
  auto bytes=QByteArray();db.getWithTag(id(0),[&](auto v){bytes=std::move(v.bytes);});db.sync();if(kind=="pending-access-prune"){auto update=DB::SettingsUpdate();update.totalSizeLimit=0;update.totalTimeLimit=3;db.updateSettings(update);until(db,[&]{return stats.full.count==0;});}auto cleaned=false;db.waitForCleaner([&]{cleaned=true;});until(db,[&]{return cleaned;});pump(db);
  auto after=stats.full.count;auto relative=error.path;relative.remove(base+'/');auto line=kind+','+QString::number(int(error.type))+','+relative+','+QString::number(before)+','+QString::number(after)+','+QString::fromLatin1(bytes.toHex())+'\n';check(report.write(line.toUtf8())==line.size(),"report row");
  db.close();db.sync();life.destroy();QCoreApplication::processEvents();
 }
}
