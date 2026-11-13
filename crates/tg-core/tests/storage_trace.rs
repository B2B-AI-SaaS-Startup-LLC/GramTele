use std::{
    cell::{Cell, RefCell},
    fmt::Write,
    rc::Rc,
    str::SplitWhitespace,
};
use tg_core::storage::{
    storage_facade::Facade, storage_shared_media::*, storage_sparse_ids_list::*,
    storage_user_photos::*,
};
use tg_rpl::{Lifetime, Producer};

type Output = Rc<RefCell<String>>;

fn opt(value: Option<i32>) -> String {
    value.map_or_else(|| "-".into(), |v| v.to_string())
}
fn ids(output: &mut String, values: impl IntoIterator<Item = impl std::fmt::Display>) {
    let values = values.into_iter().collect::<Vec<_>>();
    write!(output, " {}", values.len()).unwrap();
    for value in values {
        write!(output, " {value}").unwrap();
    }
    output.push('\n');
}
fn result(output: &Output, value: SparseIdsListResult) {
    let mut output = output.borrow_mut();
    write!(
        output,
        "R {} {} {}",
        opt(value.count),
        opt(value.skipped_before),
        opt(value.skipped_after)
    )
    .unwrap();
    ids(&mut output, value.message_ids);
}
fn update(output: &mut String, value: SparseIdsSliceUpdate) {
    write!(
        output,
        "{} {} {} {}",
        value.range.from,
        value.range.till,
        opt(value.count),
        u8::from(value.messages.is_some())
    )
    .unwrap();
    ids(output, value.messages.unwrap_or_default());
}
fn start_query(output: &Output, producer: Producer<SparseIdsListResult>, life: &Lifetime) {
    let next = output.clone();
    let done = output.clone();
    producer.start(
        move |value| result(&next, value),
        |_| unreachable!(),
        move || done.borrow_mut().push_str("D\n"),
        life,
    );
}
fn take<T: std::str::FromStr>(input: &mut SplitWhitespace<'_>) -> T {
    input.next().unwrap().parse().ok().unwrap()
}
fn media_type(index: usize) -> SharedMediaType {
    [
        SharedMediaType::Photo,
        SharedMediaType::Video,
        SharedMediaType::PhotoVideo,
        SharedMediaType::MusicFile,
        SharedMediaType::File,
        SharedMediaType::VoiceFile,
        SharedMediaType::Link,
        SharedMediaType::ChatPhoto,
        SharedMediaType::RoundVoiceFile,
        SharedMediaType::GIF,
        SharedMediaType::RoundFile,
        SharedMediaType::Pinned,
        SharedMediaType::Poll,
    ][index]
}
fn type_(input: &mut SplitWhitespace<'_>) -> SharedMediaType {
    media_type(take(input))
}
fn mask(input: &mut SplitWhitespace<'_>) -> SharedMediaTypesMask {
    let bits = take::<u32>(input);
    if bits == u32::MAX {
        return SharedMediaTypesMask::all();
    }
    let mut mask = SharedMediaTypesMask::default();
    for index in 0..SHARED_MEDIA_TYPE_COUNT {
        if bits & (1 << index) != 0 {
            mask.set(media_type(index).into());
        }
    }
    mask
}
fn range(input: &mut SplitWhitespace<'_>) -> MsgRange {
    MsgRange {
        from: take(input),
        till: take(input),
    }
}
fn count(input: &mut SplitWhitespace<'_>) -> Option<i32> {
    let value = take(input);
    (value != -1).then_some(value)
}
fn values<T: std::str::FromStr>(input: &mut SplitWhitespace<'_>) -> Vec<T> {
    (0..take::<usize>(input)).map(|_| take(input)).collect()
}

#[test]
fn native_storage_operations_match_each_event_and_query() {
    let sparse = SparseIdsList::default();
    let mut facade = Facade::default();
    let media = &mut facade.shared_media;
    let photos = &mut facade.user_photos;
    let output = Output::default();
    let life = Lifetime::new();
    let out = output.clone();
    sparse.slice_updated().on_next(
        move |value| {
            let mut out = out.borrow_mut();
            out.push_str("S ");
            update(&mut out, value);
        },
        &life,
    );
    let out = output.clone();
    media.slice_updated().on_next(
        move |value| {
            let mut out = out.borrow_mut();
            write!(
                out,
                "M {} {} {} {} ",
                value.peer_id, value.topic_root_id, value.monoforum_peer_id, value.type_ as u8
            )
            .unwrap();
            update(&mut out, value.data);
        },
        &life,
    );
    let out = output.clone();
    media.one_removed().on_next(
        move |value| {
            writeln!(
                out.borrow_mut(),
                "O {} {} {} {}",
                value.peer_id,
                value.topic_root_id,
                value.monoforum_peer_id,
                value.message_id
            )
            .unwrap();
        },
        &life,
    );
    let out = output.clone();
    media.all_removed().on_next(
        move |value| {
            writeln!(
                out.borrow_mut(),
                "A {} {} {}",
                value.peer_id,
                value.topic_root_id,
                value.monoforum_peer_id
            )
            .unwrap();
        },
        &life,
    );
    let out = output.clone();
    media.bottom_invalidated().on_next(
        move |value| {
            writeln!(out.borrow_mut(), "I {}", value.peer_id).unwrap();
        },
        &life,
    );
    let out = output.clone();
    photos.slice_updated().on_next(
        move |value| {
            let mut out = out.borrow_mut();
            write!(out, "P {} {}", value.user_id, opt(value.count)).unwrap();
            ids(&mut out, value.photo_ids);
        },
        &life,
    );

    let native = include_str!("fixtures/core-base/storage-trace-native.txt");
    let expected = native.split("@ ").skip(1).collect::<Vec<_>>();
    let commands = include_str!("fixtures/core-base/storage-trace.txt")
        .lines()
        .collect::<Vec<_>>();
    assert_eq!(commands.len(), expected.len());
    for (index, line) in commands.iter().enumerate() {
        output.borrow_mut().clear();
        writeln!(output.borrow_mut(), "{index}").unwrap();
        let mut input = line.split_whitespace();
        match input.next().unwrap() {
            "ss" => {
                let range = range(&mut input);
                let count = count(&mut input);
                sparse.add_slice(values(&mut input), range, count);
            }
            "sn" => sparse.add_new(take(&mut input)),
            "sr" => sparse.remove_one(take(&mut input)),
            "se" => sparse.add_existing(take(&mut input), range(&mut input)),
            "sa" => sparse.remove_all(),
            "si" => sparse.invalidate_bottom(),
            "sq" => {
                start_query(
                    &output,
                    sparse.query(SparseIdsListQuery {
                        around_id: take(&mut input),
                        limit_before: take(&mut input),
                        limit_after: take(&mut input),
                    }),
                    &life,
                );
                writeln!(output.borrow_mut(), "E {}", u8::from(sparse.empty())).unwrap();
            }
            "sc" => {
                let id = take(&mut input);
                let limit = take(&mut input);
                let divisor: i64 = take(&mut input);
                let calls = Cell::new(0);
                let count = sparse.count_after(id, limit, |value| {
                    calls.set(calls.get() + 1);
                    value % divisor == 0
                });
                writeln!(output.borrow_mut(), "C {} {}", opt(count), calls.get()).unwrap();
            }
            "ms" => {
                let peer_id = take(&mut input);
                let topic_root_id = take(&mut input);
                let monoforum_peer_id = take(&mut input);
                let type_ = type_(&mut input);
                let no_skip_range = range(&mut input);
                let count = count(&mut input);
                media.add_slice(SharedMediaAddSlice {
                    peer_id,
                    topic_root_id,
                    monoforum_peer_id,
                    type_,
                    no_skip_range,
                    count,
                    message_ids: values(&mut input),
                });
            }
            command @ ("mn" | "me" | "mr") => {
                let peer_id = take(&mut input);
                let topic_root_id = take(&mut input);
                let monoforum_peer_id = take(&mut input);
                let types = mask(&mut input);
                let message_id = take(&mut input);
                match command {
                    "mn" => media.add_new(SharedMediaAddNew {
                        peer_id,
                        topic_root_id,
                        monoforum_peer_id,
                        types,
                        message_id,
                    }),
                    "mr" => media.remove_one(SharedMediaRemoveOne {
                        peer_id,
                        topic_root_id,
                        monoforum_peer_id,
                        types,
                        message_id,
                    }),
                    _ => media.add_existing(SharedMediaAddExisting {
                        peer_id,
                        topic_root_id,
                        monoforum_peer_id,
                        types,
                        message_id,
                        no_skip_range: range(&mut input),
                    }),
                }
            }
            "ma" => media.remove_all(SharedMediaRemoveAll {
                peer_id: take(&mut input),
                topic_root_id: take(&mut input),
                monoforum_peer_id: take(&mut input),
                types: mask(&mut input),
            }),
            "mu" => media.unload(SharedMediaUnloadThread {
                peer_id: take(&mut input),
                topic_root_id: take(&mut input),
                monoforum_peer_id: take(&mut input),
            }),
            "mi" => media.invalidate(SharedMediaInvalidateBottom {
                peer_id: take(&mut input),
            }),
            "mq" => {
                let query = SharedMediaQuery {
                    key: SharedMediaKey {
                        peer_id: take(&mut input),
                        topic_root_id: take(&mut input),
                        monoforum_peer_id: take(&mut input),
                        type_: type_(&mut input),
                        message_id: take(&mut input),
                    },
                    limit_before: take(&mut input),
                    limit_after: take(&mut input),
                };
                result(&output, media.snapshot(&query));
                let empty = media.empty(&query.key);
                start_query(&output, media.query(query), &life);
                writeln!(output.borrow_mut(), "E {}", u8::from(empty)).unwrap();
            }
            "ps" => {
                let user = take(&mut input);
                let count = take(&mut input);
                photos.add_slice(user, values(&mut input), count);
            }
            command @ ("pn" | "pb" | "pr" | "pa" | "px") => {
                let user = take(&mut input);
                let id = take(&mut input);
                match command {
                    "pn" => photos.add_new(user, id),
                    "pb" => photos.set_back(user, id),
                    "pr" => photos.remove_one(user, id),
                    "pa" => photos.remove_after(user, id),
                    _ => photos.replace(user, id, take(&mut input)),
                }
            }
            "pq" => {
                let query = UserPhotosQuery {
                    key: UserPhotosKey {
                        user_id: take(&mut input),
                        photo_id: take(&mut input),
                        back: take::<u8>(&mut input) != 0,
                    },
                    limit_before: take(&mut input),
                    limit_after: take(&mut input),
                };
                let out = output.clone();
                let done = output.clone();
                photos.query(query).start(
                    move |value| {
                        let mut out = out.borrow_mut();
                        write!(
                            out,
                            "R {} {} {}",
                            opt(value.count),
                            opt(value.skipped_before),
                            value.skipped_after
                        )
                        .unwrap();
                        ids(&mut out, value.photo_ids);
                    },
                    |_| unreachable!(),
                    move || done.borrow_mut().push_str("D\n"),
                    &life,
                );
            }
            command => panic!("unknown command {command}"),
        }
        assert!(input.next().is_none(), "unused input for {line}");
        assert_eq!(
            &*output.borrow(),
            expected[index],
            "operation {index}: {line}"
        );
    }
}
