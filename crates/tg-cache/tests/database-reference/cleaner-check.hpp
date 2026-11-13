#include <unistd.h>
void cleaner_check(const QString &root) {
 check(!QFile::exists(root),"new cleaner directory required");
 for(const auto &dir:{"0/target","0/keep","2/nested",".hidden"})check(QDir().mkpath(root+'/'+dir),"cleaner mkdir");
 for(const auto &name:{"0/target/deleted","0/keep/kept","2/nested/readonly",".hidden/kept"}){QFile f(root+'/'+name);check(f.open(QIODevice::WriteOnly)&&f.write("x",1)==1,"cleaner fixture");}
 check(QFile::setPermissions(root+"/2/nested/readonly",QFile::ReadOwner|QFile::ReadGroup|QFile::ReadOther),"readonly");
 auto a=(root+"/link").toUtf8();auto b=(root+"/2/nested-link").toUtf8();check(::symlink("0/target",a.constData())==0&&::symlink("../0/keep",b.constData())==0,"cleaner links");
 check(Storage::Cache::details::WriteVersionValue(root+'/',0),"cleaner version");DB db(root,settings("stores"));auto error=Storage::Cache::Error();db.open(key(),[&](auto e){error=e;});db.sync();check(error.type==Storage::Cache::Error::Type::None,"cleaner open");auto finished=false;db.waitForCleaner([&]{finished=true;});until(db,[&]{return finished;});
 std::cout<<"cleaner,"<<QFileInfo(root+"/2").exists()<<','<<QFileInfo(root+"/.hidden/kept").exists()<<','<<QFileInfo(root+"/0/keep/kept").exists()<<','<<QFileInfo(root+"/0/target/deleted").exists()<<','<<QFileInfo(root+"/link").isSymLink()<<','<<QFileInfo(root+"/0/target").isDir()<<'\n';db.close();db.sync();
}
