use std::{cell::RefCell, rc::Rc};
use tg_core::storage::{
    storage_facade::Facade, storage_shared_media::*, storage_sparse_ids_list::*,
    storage_user_photos::*,
};
fn snapshot(list: &SparseIdsList, id: i64) -> SparseIdsListResult {
    list.snapshot(&SparseIdsListQuery {
        around_id: id,
        limit_before: 20,
        limit_after: 20,
    })
}
#[test]
fn sparse_merge_counts_and_known_ends() {
    let l = SparseIdsList::default();
    assert!(l.empty());
    assert_eq!(l.count_after(0, 10, |_| true), None);
    l.add_slice(vec![10, 20], MsgRange { from: 0, till: 20 }, Some(5));
    l.add_slice(
        vec![40, 50],
        MsgRange {
            from: 40,
            till: SERVER_MAX_MSG_ID,
        },
        None,
    );
    assert!(snapshot(&l, 30).message_ids.is_empty());
    l.add_existing(30, MsgRange { from: 20, till: 40 });
    let r = snapshot(&l, 30);
    assert_eq!(r.message_ids, [10, 20, 30, 40, 50].into());
    assert_eq!(
        (r.count, r.skipped_before, r.skipped_after),
        (Some(5), Some(0), Some(0))
    );
    assert_eq!(l.count_after(20, 2, |id| id % 20 == 0), None);
    assert_eq!(l.count_after(20, 3, |id| id % 20 == 0), Some(1));
    l.add_new(60);
    assert_eq!(snapshot(&l, 30).count, Some(6));
    l.remove_one(999);
    assert_eq!(snapshot(&l, 30).count, Some(5));
    l.invalidate_bottom();
    assert_eq!(l.count_after(0, 20, |_| true), None);
    assert_eq!(snapshot(&l, 30).count, None);
    l.remove_all();
    assert!(l.empty());
    assert_eq!(snapshot(&l, 1).count, Some(0));
}
#[test]
fn sparse_ayu_reversed_range_and_update_precedes_expansion() {
    let l = SparseIdsList::default();
    let events = Rc::new(RefCell::new(Vec::new()));
    let out = events.clone();
    let _life = l
        .slice_updated()
        .on_next_returning(move |v| out.borrow_mut().push(v));
    l.add_slice(vec![5], MsgRange { from: 5, till: 5 }, Some(1));
    assert_eq!(events.borrow()[0].range, MsgRange { from: 5, till: 5 });
    assert_eq!(snapshot(&l, 4).message_ids, [5].into());
    l.add_slice(vec![6], MsgRange { from: 10, till: 2 }, Some(9));
    let e = events.borrow();
    assert_eq!(e.len(), 2);
    assert_eq!(e[1].messages, None);
    assert_eq!(e[1].count, Some(1));
}
#[test]
fn sparse_query_limits_and_lazy_snapshot() {
    let l = SparseIdsList::default();
    let producer = l.query(SparseIdsListQuery {
        around_id: 20,
        limit_before: 1,
        limit_after: 0,
    });
    l.add_slice(
        vec![10, 20, 30],
        MsgRange {
            from: 0,
            till: SERVER_MAX_MSG_ID,
        },
        Some(8),
    );
    let result = Rc::new(RefCell::new(Vec::new()));
    let out = result.clone();
    let _life = producer.on_next_returning(move |v| out.borrow_mut().push(v));
    let r = &result.borrow()[0];
    assert_eq!(r.message_ids, [10, 20].into());
    assert_eq!((r.skipped_before, r.skipped_after), (Some(0), Some(1)));
}
fn photos(p: &UserPhotos, user: u64, id: u64, back: bool) -> Vec<UserPhotosResult> {
    let out = Rc::new(RefCell::new(Vec::new()));
    let copy = out.clone();
    let _life = p
        .query(UserPhotosQuery {
            key: UserPhotosKey {
                user_id: user,
                photo_id: id,
                back,
            },
            limit_before: 100,
            limit_after: 100,
        })
        .on_next_returning(move |r| copy.borrow_mut().push(r));
    let result = out.borrow().clone();
    result
}
#[test]
fn photo_order_back_intermediate_notifications_replace_remove() {
    let mut p = UserPhotos::default();
    assert!(photos(&p, 1, 0, false).is_empty());
    let events = Rc::new(RefCell::new(Vec::new()));
    let out = events.clone();
    let _life = p
        .slice_updated()
        .on_next_returning(move |v| out.borrow_mut().push(v));
    p.add_slice(1, vec![3, 2, 1], 3);
    assert_eq!(photos(&p, 1, 2, false)[0].photo_ids, Vec::from([1, 2, 3]));
    p.set_back(1, 9);
    p.add_new(1, 4);
    assert_eq!(events.borrow().len(), 4);
    assert_eq!(events.borrow()[2].photo_ids, Vec::from([1, 2, 3]));
    assert_eq!(
        photos(&p, 1, 2, false)[0].photo_ids,
        Vec::from([9, 1, 2, 3, 4])
    );
    p.replace(1, 9, 8);
    p.remove_after(1, 3);
    assert_eq!(photos(&p, 1, 1, false)[0].photo_ids, Vec::from([8, 1, 2]));
    p.remove_one(1, 55);
    assert_eq!(photos(&p, 1, 1, false)[0].count, None);
    p.remove_after(1, 99);
    assert!(photos(&p, 1, 1, false).is_empty());
    assert_eq!(photos(&p, 1, 1, true)[0].photo_ids, Vec::from([8]));
}
#[test]
fn media_scopes_masks_facade_and_events() {
    let mut f = Facade::default();
    let query = |topic| SharedMediaQuery {
        key: SharedMediaKey {
            peer_id: 1,
            topic_root_id: topic,
            monoforum_peer_id: 0,
            type_: SharedMediaType::Photo,
            message_id: 10,
        },
        limit_before: 100,
        limit_after: 100,
    };
    let key = query(0).key;
    assert!(f.empty(&key));
    for topic in [0, 2] {
        f.add_slice(SharedMediaAddSlice {
            peer_id: 1,
            topic_root_id: topic,
            monoforum_peer_id: 0,
            message_ids: vec![10],
            no_skip_range: MsgRange {
                from: 0,
                till: SERVER_MAX_MSG_ID,
            },
            type_: SharedMediaType::Photo,
            count: Some(1),
        });
    }
    f.add_new(SharedMediaAddNew {
        peer_id: 1,
        topic_root_id: 2,
        monoforum_peer_id: 0,
        message_id: 20,
        types: 1,
    });
    assert_eq!(f.snapshot(&query(2)).count, Some(2));
    f.remove_one(SharedMediaRemoveOne {
        peer_id: 1,
        topic_root_id: 0,
        monoforum_peer_id: 0,
        message_id: 10,
        types: 1,
    });
    assert_eq!(f.snapshot(&query(0)).count, Some(1));
    assert_eq!(f.snapshot(&query(2)).count, Some(2));
    f.invalidate(SharedMediaInvalidateBottom { peer_id: 1 });
    assert_eq!(f.snapshot(&query(2)).count, None);
    f.remove_all(SharedMediaRemoveAll {
        peer_id: 1,
        topic_root_id: 0,
        monoforum_peer_id: 0,
        types: 1,
    });
    assert!(f.empty(&key));
    f.unload(SharedMediaUnloadThread {
        peer_id: 1,
        topic_root_id: 2,
        monoforum_peer_id: 0,
    });
    assert_eq!(f.snapshot(&query(2)).count, None);
    f.user_photos.add_new(1, 50);
    assert_eq!(
        photos(&f.user_photos, 1, 50, false)[0].photo_ids,
        Vec::from([50])
    );
}
#[test]
fn sparse_native_reference() {
    let l = SparseIdsList::default();
    let mut actual = String::new();
    let mut dump = |id| {
        let r = snapshot(&l, id);
        let opt = |v: Option<i32>| v.map_or_else(|| "-".into(), |n| n.to_string());
        actual += &format!(
            "{} {} {}:",
            opt(r.count),
            opt(r.skipped_before),
            opt(r.skipped_after)
        );
        for id in r.message_ids {
            actual += &format!(" {id}");
        }
        actual += '\n'.to_string().as_str();
    };
    dump(1);
    l.add_slice(vec![10, 20], MsgRange { from: 0, till: 20 }, Some(5));
    dump(20);
    l.add_slice(
        vec![40, 50],
        MsgRange {
            from: 40,
            till: SERVER_MAX_MSG_ID,
        },
        None,
    );
    dump(30);
    l.add_existing(30, MsgRange { from: 20, till: 40 });
    dump(30);
    l.add_new(60);
    dump(30);
    l.remove_one(999);
    dump(30);
    l.invalidate_bottom();
    dump(30);
    l.remove_all();
    dump(1);
    l.add_new(1 << 40);
    dump(1 << 40);
    l.add_slice(vec![7], MsgRange { from: 20, till: 2 }, Some(9));
    dump(1 << 40);
    assert_eq!(actual, include_str!("fixtures/core-base/sparse-native.txt"));
}
#[test]
fn photos_native_reference(){let mut p=UserPhotos::default();let lines=Rc::new(RefCell::new(String::new()));let out=lines.clone();let _life=p.slice_updated().on_next_returning(move|u|{use std::fmt::Write;let mut text=out.borrow_mut();write!(text,"P {}:",u.count.map_or_else(||"-".into(),|v|v.to_string())).unwrap();for id in u.photo_ids{write!(text," {id}").unwrap();}text.push('\n');});p.add_slice(1,vec![3,2,1],3);p.set_back(1,9);p.add_new(1,4);p.replace(1,9,8);p.remove_after(1,3);p.remove_one(1,55);p.remove_after(1,99);assert_eq!(*lines.borrow(),include_str!("fixtures/core-base/photos-native.txt"));}
