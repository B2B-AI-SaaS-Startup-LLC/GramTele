//! Photo list ordering is oldest-to-newest, with the special back photo at front.
use std::collections::{BTreeMap, VecDeque};
use tg_rpl::{EventStream, Lifetime, Producer, Variable};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UserPhotosKey {
    pub user_id: u64,
    pub photo_id: u64,
    pub back: bool,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct UserPhotosQuery {
    pub key: UserPhotosKey,
    pub limit_before: i32,
    pub limit_after: i32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UserPhotosResult {
    pub count: Option<i32>,
    pub skipped_before: Option<i32>,
    pub skipped_after: i32,
    pub photo_ids: VecDeque<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserPhotosSliceUpdate {
    pub user_id: u64,
    pub photo_ids: VecDeque<u64>,
    pub count: Option<i32>,
}
#[derive(Clone, Default, PartialEq)]
struct State {
    count: Option<i32>,
    ids: VecDeque<u64>,
    back: u64,
}
struct List {
    state: std::rc::Rc<Variable<State>>,
    updates: EventStream<State>,
}
impl Default for List {
    fn default() -> Self {
        Self {
            state: std::rc::Rc::new(Variable::new(State::default())),
            updates: EventStream::new(),
        }
    }
}
impl List {
    fn send_update(&self, s: State) {
        self.state.set(s.clone());
        self.updates.fire(s);
    }
    fn remove_one(&self, id: u64) {
        let mut s = self.state.current();
        if let Some(p) = s.ids.iter().position(|v| *v == id) {
            s.ids.remove(p);
            if let Some(c) = &mut s.count {
                *c -= 1;
            }
        } else {
            s.count = None;
        }
        self.send_update(s);
    }
    fn detach_back(&self) {
        let id = self.state.current().back;
        if id != 0 {
            self.remove_one(id);
        }
    }
    fn attach_back(s: &mut State) {
        if s.back != 0 {
            s.ids.push_front(s.back);
            if let Some(c) = &mut s.count {
                *c += 1;
            }
        }
    }
    fn set_back(&self, id: u64) {
        if self.state.current().back != id {
            self.detach_back();
            let mut s = self.state.current();
            s.back = id;
            Self::attach_back(&mut s);
            self.send_update(s);
        }
    }
    fn add_new(&self, id: u64) {
        if !self.state.current().ids.contains(&id) {
            self.detach_back();
            let mut s = self.state.current();
            s.ids.push_back(id);
            if let Some(c) = &mut s.count {
                *c += 1;
            }
            Self::attach_back(&mut s);
            self.send_update(s);
        }
    }
    fn add_slice(&self, ids: Vec<u64>, count: i32) {
        self.detach_back();
        let mut s = self.state.current();
        for id in &ids {
            if !s.ids.contains(id) {
                s.ids.push_front(*id);
            }
        }
        s.count = Some(count);
        if (count as usize) < s.ids.len() || ids.is_empty() {
            s.count = Some(s.ids.len() as i32);
        }
        Self::attach_back(&mut s);
        self.send_update(s);
    }
    fn remove_after(&self, id: u64) {
        let mut s = self.state.current();
        if let Some(p) = s.ids.iter().position(|v| *v == id) {
            if let Some(c) = &mut s.count {
                *c -= (s.ids.len() - p) as i32;
            }
            s.ids.truncate(p);
        } else {
            s.count = None;
            s.ids.clear();
        }
        self.send_update(s);
    }
    fn replace(&self, old: u64, new: u64) {
        let mut s = self.state.current();
        if let Some(p) = s.ids.iter().position(|v| *v == old) {
            s.ids[p] = new;
        }
        if s.back == old {
            s.back = new;
        }
        self.send_update(s);
    }
    fn query(&self, q: UserPhotosQuery) -> Producer<UserPhotosResult> {
        {
            let state = self.state.clone();
            tg_rpl::deferred(move || tg_rpl::single(state.current()))
                .map(move |s| {
                    assert!(q.limit_before >= 0 && q.limit_after >= 0);
                    let mut r = UserPhotosResult {
                        count: s.count,
                        ..Default::default()
                    };
                    if let Some(p) = s.ids.iter().position(|id| *id == q.key.photo_id) {
                        let before = p.min(q.limit_before as usize);
                        let after = (s.ids.len() - p).min(q.limit_after as usize + 1);
                        r.photo_ids = s.ids.range(p - before..p + after).copied().collect();
                        // Original overwrites its count-based skippedBefore calculation immediately.
                        r.skipped_before = Some((p - before) as i32);
                        r.skipped_after = (s.ids.len() - p - after) as i32;
                        Some(r)
                    } else if q.key.back && s.back != 0 {
                        r.photo_ids.push_front(s.back);
                        r.count = Some(1);
                        Some(r)
                    } else {
                        s.count.map(|_| r)
                    }
                })
                .filter_optional()
        }
    }
}
pub struct UserPhotos {
    lifetime: Lifetime,
    updates: EventStream<UserPhotosSliceUpdate>,
    lists: BTreeMap<u64, List>,
}
impl Default for UserPhotos {
    fn default() -> Self {
        Self {
            lists: BTreeMap::new(),
            updates: EventStream::new(),
            lifetime: Lifetime::new(),
        }
    }
}
impl UserPhotos {
    fn enforce_lists(&mut self, user: u64) -> &List {
        self.lists.entry(user).or_insert_with(|| {
            let list = List::default();
            list.updates
                .events()
                .map(move |s| UserPhotosSliceUpdate {
                    user_id: user,
                    photo_ids: s.ids,
                    count: s.count,
                })
                .start_to_stream(&self.updates, &self.lifetime);
            list
        })
    }
    pub fn set_back(&mut self, user: u64, id: u64) {
        self.enforce_lists(user).set_back(id);
    }
    pub fn add_new(&mut self, user: u64, id: u64) {
        self.enforce_lists(user).add_new(id);
    }
    pub fn add_slice(&mut self, user: u64, ids: Vec<u64>, count: i32) {
        self.enforce_lists(user).add_slice(ids, count);
    }
    pub fn remove_one(&self, user: u64, id: u64) {
        if let Some(list) = self.lists.get(&user) {
            list.remove_one(id);
        }
    }
    pub fn remove_after(&self, user: u64, id: u64) {
        if let Some(list) = self.lists.get(&user) {
            list.remove_after(id);
        }
    }
    pub fn replace(&self, user: u64, old: u64, new: u64) {
        if let Some(list) = self.lists.get(&user) {
            list.replace(old, new);
        }
    }
    pub fn query(&self, q: UserPhotosQuery) -> Producer<UserPhotosResult> {
        self.lists
            .get(&q.key.user_id)
            .map_or_else(tg_rpl::complete, |list| list.query(q))
    }
    pub fn slice_updated(&self) -> Producer<UserPhotosSliceUpdate> {
        self.updates.events()
    }
}
