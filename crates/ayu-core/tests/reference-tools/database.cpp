// Executes the original sqlite_orm declarations, migrations and query functions.
#include "ayu/data/ayu_database.cpp"
#include <filesystem>
#include <iostream>
namespace base::unixtime { TimeId now() { return 1700000000; } }
int main(int argc,char **argv) {
 if(argc!=3)return 1;std::filesystem::current_path(argv[2]);
 if(std::string(argv[1])=="create"){
 std::filesystem::create_directories("tdata");AyuDatabase::initialize();
 DeletedMessage d{};d.userId=9223372036854775807LL;d.dialogId=(-9223372036854775807LL-1);d.messageId=2147483647;d.text="Привет 🌏\n";d.mediaPath="";d.replySerialized={0,127,-128,-1};d.textEntities={1,0,2};d.mimeType="application/test";d.replyForumTopic=true;AyuDatabase::addDeletedMessage(d);
 EditedMessage e{};static_cast<AyuMessageBase&>(e)=d;e.messageId=(-2147483647-1);AyuDatabase::addEditedMessage(e);e.fakeId=7;storage.replace(e);
 DeletedDialog dialog{};dialog.userId=42;dialog.dialogId=-9;dialog.folderId=nullptr;storage.insert(dialog);dialog.folderId=std::make_unique<int>(0);storage.insert(dialog);
 RegexFilter filter{{'a','\0'},"é\\_%",true,false,true,std::nullopt};AyuDatabase::addRegexFilter(filter);filter.id={'b','\0'};filter.dialogId=-123;AyuDatabase::addRegexFilter(filter);
 AyuDatabase::addRegexExclusion({0,-123,{'a','\0'}});
 storage.insert(SpyMessageRead{0,42,-1,0,1700000000});storage.replace(SpyMessageRead{9223372036854775807LL,(-9223372036854775807LL-1),9223372036854775807LL,(-2147483647-1),2147483647});storage.insert(SpyMessageContentsRead{0,42,-1,2147483647,0});
 }else if(std::string(argv[1])=="read"){
 AyuDatabase::initialize();const auto f=AyuDatabase::getById({'a','\0'});std::cerr<<"getById a: bytes="<<f.id.size()<<" text="<<f.text<<"\n";std::cout<<storage.count<DeletedMessage>()<<" "<<storage.count<EditedMessage>()<<" "<<storage.count<DeletedDialog>()<<" "<<storage.count<RegexFilter>()<<" "<<storage.count<RegexFilterGlobalExclusion>()<<" "<<storage.count<SpyMessageRead>()<<" "<<storage.count<SpyMessageContentsRead>()<<"\n";
 auto dump=[&]<typename T>() {for(const auto &row:storage.get_all<T>()){std::cout<<storage.dump(row)<<"\n";}};
 dump.operator()<SchemaVersion>();dump.operator()<DeletedMessage>();dump.operator()<EditedMessage>();dump.operator()<DeletedDialog>();dump.operator()<RegexFilter>();dump.operator()<RegexFilterGlobalExclusion>();dump.operator()<SpyMessageRead>();dump.operator()<SpyMessageContentsRead>();
 }else return 2;
}
