#include "storage/storage_sparse_ids_list.h"
#include "storage/storage_user_photos.h"
void dump(Storage::SparseIdsList &l, MsgId around) {
 const auto r=l.snapshot({around,20,20});
 const auto opt=[](std::optional<int> v){return v?std::to_string(*v):"-";};
 std::cout<<opt(r.count)<<' '<<opt(r.skippedBefore)<<' '<<opt(r.skippedAfter)<<':';
 for(auto id:r.messageIds)std::cout<<' '<<id;
 std::cout<<'\n';
}
void photos(){Storage::UserPhotos p;rpl::lifetime life;p.sliceUpdated()|rpl::on_next([](const Storage::UserPhotosSliceUpdate &u){std::cout<<"P "<<(u.count?std::to_string(*u.count):"-")<<":";for(auto id:*u.photoIds)std::cout<<" "<<id;std::cout<<"\n";},life);p.add(Storage::UserPhotosAddSlice(1,{3,2,1},3));p.add(Storage::UserPhotosSetBack(1,9));p.add(Storage::UserPhotosAddNew(1,4));p.replace(Storage::UserPhotosReplace(1,9,8));p.remove(Storage::UserPhotosRemoveAfter(1,3));p.remove(Storage::UserPhotosRemoveOne(1,55));p.remove(Storage::UserPhotosRemoveAfter(1,99));}
int main(int argc,char**){if(argc>1){photos();return 0;}Storage::SparseIdsList l;dump(l,1);l.addSlice({10,20},{0,20},5);dump(l,20);l.addSlice({40,50},{40,ServerMaxMsgId},std::nullopt);dump(l,30);l.addExisting(30,{20,40});dump(l,30);l.addNew(60);dump(l,30);l.removeOne(999);dump(l,30);l.invalidateBottom();dump(l,30);l.removeAll();dump(l,1);l.addNew(1LL<<40);dump(l,1LL<<40);l.addSlice({7},{20,2},9);dump(l,1LL<<40);}
