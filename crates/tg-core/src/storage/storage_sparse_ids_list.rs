//! storage_sparse_ids_list: ranges are closed; zero/MAX certify known ends.
use std::collections::BTreeSet;
use tg_rpl::{EventStream, Producer, Variable};
pub const SERVER_MAX_MSG_ID: i64 = 1 << 56;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MsgRange {
    pub from: i64,
    pub till: i64,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct SparseIdsListQuery {
    pub around_id: i64,
    pub limit_before: i32,
    pub limit_after: i32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SparseIdsListResult {
    pub count: Option<i32>,
    pub skipped_before: Option<i32>,
    pub skipped_after: Option<i32>,
    pub message_ids: BTreeSet<i64>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SparseIdsSliceUpdate {
    pub messages: Option<BTreeSet<i64>>,
    pub range: MsgRange,
    pub count: Option<i32>,
}
#[derive(Clone, Default, PartialEq)]
struct Slice {
    messages: BTreeSet<i64>,
    range: MsgRange,
}
#[derive(Clone, Default, PartialEq)]
struct State {
    count: Option<i32>,
    slices: Vec<Slice>,
}
pub struct SparseIdsList {
    state: std::rc::Rc<Variable<State>>,
    updated: EventStream<SparseIdsSliceUpdate>,
}
impl Default for SparseIdsList {
    fn default() -> Self {
        Self {
            state: std::rc::Rc::new(Variable::new(State::default())),
            updated: EventStream::new(),
        }
    }
}
impl SparseIdsList {
    fn add_range(
        &self,
        messages: impl IntoIterator<Item = i64>,
        range: MsgRange,
        count: Option<i32>,
        increment: bool,
    ) {
        assert!(!increment || count.is_none());
        let mut s = self.state.current();
        let mut update = SparseIdsSliceUpdate::default();
        let mut added = 0;
        // AyuGram: reversed range is a no-op for slices, but addRange still applies count/emits.
        if range.from <= range.till {
            let messages = messages.into_iter().collect::<BTreeSet<_>>();
            if range.from != range.till || !messages.is_empty() {
                let from = s.slices.partition_point(|v| v.range.till < range.from);
                let till = s.slices.partition_point(|v| v.range.from <= range.till);
                if from < till {
                    let before = s.slices[from].messages.len();
                    let mut joined = s.slices[from].clone();
                    joined.messages.extend(messages);
                    joined.range.from = joined.range.from.min(range.from);
                    joined.range.till = joined.range.till.max(range.till);
                    for v in &s.slices[from + 1..till] {
                        joined.messages.extend(v.messages.iter().copied());
                        joined.range.from = joined.range.from.min(v.range.from);
                        joined.range.till = joined.range.till.max(v.range.till);
                    }
                    added = joined.messages.len() as i32 - before as i32;
                    s.slices.splice(from..till, [joined]);
                } else {
                    added = messages.len() as i32;
                    s.slices.insert(from, Slice { messages, range });
                }
                update.messages = Some(s.slices[from].messages.clone());
                update.range = s.slices[from].range;
            }
        }
        if count.is_some() {
            s.count = count;
        } else if increment && added > 0 {
            if let Some(v) = &mut s.count {
                *v += added;
            }
        }
        if s.slices.len() == 1 {
            let v = &mut s.slices[0];
            if s.count.is_some_and(|c| v.messages.len() >= c as usize) {
                v.range = MsgRange {
                    from: 0,
                    till: SERVER_MAX_MSG_ID,
                };
            }
            if v.range
                == (MsgRange {
                    from: 0,
                    till: SERVER_MAX_MSG_ID,
                })
            {
                s.count = Some(v.messages.len() as i32);
            }
        }
        if let (Some(c), Some(ids)) = (&mut s.count, &update.messages) {
            *c = (*c).max(ids.len() as i32);
        }
        update.count = s.count;
        self.state.set(s);
        self.updated.fire(update);
    }
    pub fn add_new(&self, id: i64) {
        self.add_range(
            [id],
            MsgRange {
                from: id,
                till: SERVER_MAX_MSG_ID,
            },
            None,
            true,
        );
    }
    pub fn add_existing(&self, id: i64, range: MsgRange) {
        self.add_range([id], range, None, false);
    }
    pub fn add_slice(&self, ids: Vec<i64>, range: MsgRange, count: Option<i32>) {
        self.add_range(ids, range, count, false);
    }
    pub fn remove_one(&self, id: i64) {
        let mut s = self.state.current();
        let i = s.slices.partition_point(|v| v.range.till < id);
        if let Some(slice) = s.slices.get_mut(i).filter(|v| v.range.from <= id) {
            slice.messages.remove(&id);
        }
        if let Some(c) = &mut s.count {
            if *c > 0 {
                *c -= 1;
            }
        }
        self.state.set(s);
    }
    pub fn remove_all(&self) {
        self.state.set(State {
            count: Some(0),
            slices: vec![Slice {
                messages: BTreeSet::new(),
                range: MsgRange {
                    from: 0,
                    till: SERVER_MAX_MSG_ID,
                },
            }],
        });
    }
    pub fn invalidate_bottom(&self) {
        let mut s = self.state.current();
        if let Some(v) = s
            .slices
            .last_mut()
            .filter(|v| v.range.till == SERVER_MAX_MSG_ID)
        {
            v.range.till = v.messages.last().copied().unwrap_or(v.range.from);
        }
        s.count = None;
        self.state.set(s);
    }
    pub fn empty(&self) -> bool {
        self.state
            .current()
            .slices
            .iter()
            .all(|v| v.messages.is_empty())
    }
    pub fn count_after(&self, till: i64, limit: i32, counts: impl Fn(i64) -> bool) -> Option<i32> {
        let s = self.state.current();
        let v = s.slices.last()?;
        if v.range.till != SERVER_MAX_MSG_ID || (v.range.from > till && v.range.from != 0) {
            return None;
        }
        let ids = v
            .messages
            .range((std::ops::Bound::Excluded(till), std::ops::Bound::Unbounded))
            .copied()
            .collect::<Vec<_>>();
        if ids.len() as i32 > limit {
            return None;
        }
        Some(ids.into_iter().filter(|id| counts(*id)).count() as i32)
    }
    pub fn snapshot(&self, q: &SparseIdsListQuery) -> SparseIdsListResult {
        snapshot(&self.state.current(), q)
    }
    pub fn query(&self, q: SparseIdsListQuery) -> Producer<SparseIdsListResult> {
        {
            let state = self.state.clone();
            tg_rpl::deferred(move || tg_rpl::single(snapshot(&state.current(), &q)))
                .filter(|r| !r.message_ids.is_empty() || r.count.is_some())
        }
    }
    pub fn slice_updated(&self) -> Producer<SparseIdsSliceUpdate> {
        self.updated.events()
    }
}
fn snapshot(s: &State, q: &SparseIdsListQuery) -> SparseIdsListResult {
    assert!(q.limit_before >= 0 && q.limit_after >= 0);
    let slice = if q.around_id != 0 {
        s.slices
            .iter()
            .find(|v| v.range.till >= q.around_id)
            .filter(|v| v.range.from <= q.around_id)
    } else {
        None
    };
    let Some(v) = slice else {
        return SparseIdsListResult {
            count: s.count,
            ..Default::default()
        };
    };
    let ids = v.messages.iter().copied().collect::<Vec<_>>();
    let p = ids.partition_point(|id| *id < q.around_id);
    let before = p.min(q.limit_before as usize);
    let after = (ids.len() - p).min(q.limit_after as usize + 1);
    let selected = ids[p - before..p + after]
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut result = SparseIdsListResult {
        count: s.count,
        skipped_before: (v.range.from == 0).then_some((p - before) as i32),
        skipped_after: (v.range.till == SERVER_MAX_MSG_ID)
            .then_some((ids.len() - p - after) as i32),
        message_ids: selected,
    };
    if let Some(c) = s.count {
        if result.skipped_before.is_none() {
            if let Some(a) = result.skipped_after {
                result.skipped_before = Some(c - a - result.message_ids.len() as i32);
            }
        } else if let (None,Some(before))=(result.skipped_after,result.skipped_before) {
            result.skipped_after = Some(c-before-result.message_ids.len() as i32);
        }
    }
    result
}
