//! Original received-id window: insertion does not shrink until explicitly asked.
use std::collections::BTreeMap;
pub const IDS_BUFFER_SIZE: usize = 400;
#[derive(Debug, Eq, PartialEq)]
pub enum State {
    NotFound,
    NeedsAck,
    NoAckNeeded,
}
#[derive(Debug, Eq, PartialEq)]
pub enum Result {
    Success,
    Duplicate,
    TooOld,
}
#[derive(Default)]
pub struct ReceivedIdsManager {
    ids_need_ack: BTreeMap<u64, bool>,
}
impl ReceivedIdsManager {
    pub fn register_msg_id(&mut self, id: u64, need_ack: bool) -> Result {
        if self.ids_need_ack.contains_key(&id) {
            Result::Duplicate
        } else if self.ids_need_ack.len() < IDS_BUFFER_SIZE || id > self.min() {
            self.ids_need_ack.insert(id, need_ack);
            Result::Success
        } else {
            Result::TooOld
        }
    }
    pub fn min(&self) -> u64 {
        self.ids_need_ack.first_key_value().map_or(0, |(&id, _)| id)
    }
    pub fn max(&self) -> u64 {
        self.ids_need_ack.last_key_value().map_or(0, |(&id, _)| id)
    }
    pub fn lookup(&self, id: u64) -> State {
        match self.ids_need_ack.get(&id) {
            None => State::NotFound,
            Some(true) => State::NeedsAck,
            Some(false) => State::NoAckNeeded,
        }
    }
    pub fn shrink(&mut self) {
        while self.ids_need_ack.len() > IDS_BUFFER_SIZE {
            self.ids_need_ack.pop_first();
        }
    }
    pub fn clear(&mut self) {
        self.ids_need_ack.clear();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window() {
        let mut m = ReceivedIdsManager::default();
        for id in 1..=400 {
            assert_eq!(m.register_msg_id(id, true), Result::Success);
        }
        assert_eq!(m.register_msg_id(1, false), Result::Duplicate);
        assert_eq!(m.register_msg_id(0, true), Result::TooOld);
        assert_eq!(m.register_msg_id(401, false), Result::Success);
        assert_eq!(m.min(), 1);
        m.shrink();
        assert_eq!(m.min(), 2);
        assert_eq!(m.max(), 401);
        assert_eq!(m.lookup(401), State::NoAckNeeded);
        assert_eq!(m.lookup(2), State::NeedsAck);
        assert_eq!(m.lookup(1), State::NotFound);
        m.clear();
        assert_eq!(m.min(), 0);
    }
}
