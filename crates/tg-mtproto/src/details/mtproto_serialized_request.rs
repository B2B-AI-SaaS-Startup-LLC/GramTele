//! Session-owned request metadata; the serialized body starts at word 8.
use crate::core_types::Random;
use tg_tl::{Buffer, Serialize};
#[derive(Default, Debug)]
pub struct SerializedRequest {
    pub words: Buffer,
    pub after: Option<RequestHandle>,
    pub last_sent_time: i64,
    pub request_id: i32,
    pub needs_layer: bool,
    pub force_send_in_container: bool,
}
impl SerializedRequest {
    pub const MESSAGE_BODY_POSITION: usize = 8;
    pub fn prepare(size: usize, reserve_size: usize, now_ms: i64) -> Self {
        assert!(size > 0);
        let mut words = Vec::with_capacity(8 + size.max(reserve_size));
        words.resize(8, 0);
        words[7] = (size * 4) as i32;
        Self {
            words,
            last_sent_time: now_ms,
            ..Self::default()
        }
    }
    pub fn serialize<T: Serialize>(request: &T, now_ms: i64) -> Self {
        let body = tg_tl::serialize(request);
        let mut result = Self::prepare(body.len(), 0, now_ms);
        result.words.extend(body);
        result
    }
    pub fn set_msg_id(&mut self, id: u64) {
        assert!(self.words.len() > 8);
        self.words[4] = id as i32;
        self.words[5] = (id >> 32) as i32;
    }
    pub fn get_msg_id(&self) -> u64 {
        self.words[4] as u32 as u64 | ((self.words[5] as u32 as u64) << 32)
    }
    pub fn set_seq_no(&mut self, no: u32) {
        self.words[6] = no as i32;
    }
    pub fn get_seq_no(&self) -> u32 {
        self.words[6] as u32
    }
    pub fn body(&self) -> &[i32] {
        if self.words.is_empty() {
            return &[];
        }
        &self.words[8..8 + self.words[7] as usize / 4]
    }
    pub fn message_size(&self) -> usize {
        4 + self.words[7] as usize / 4
    }
    pub fn add_padding(&mut self, for_auth_key_inner: bool, random: &mut impl Random) {
        let size = self.words[7] as usize / 4;
        let mut padding = (4 - ((8 + size) % 4)) % 4;
        if !for_auth_key_inner {
            if padding < 3 {
                padding += 4;
            }
            let mut b = [0];
            random.fill(&mut b);
            padding += ((b[0] & 15) as usize) * 4;
        }
        let full = 8 + size + padding;
        if self.words.len() != full {
            self.words.resize(full, 0);
            for word in &mut self.words[full - padding..] {
                *word = random.u32() as i32;
            }
        }
    }
    pub fn need_ack(&self) -> bool {
        !matches!(
            self.words[8] as u32,
            0x73f1f8dc
                | 0x62d6b459
                | 0x9299359f
                | 0xa7eff811
                | 0x8cc0d131
                | 0x04deb57d
                | 0x276d3ec6
                | 0x809db6df
        )
    }
}
impl Serialize for SerializedRequest {
    fn write(&self, to: &mut Buffer) {
        to.extend_from_slice(self.body());
    }
}

/// Shared request identity without a RefCell per request. Session maps copy this
/// handle; the owner holds one mutable RequestData in its central store.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct RequestHandle(u64);
#[derive(Default)]
pub struct RequestStore {
    next: u64,
    data: std::collections::BTreeMap<RequestHandle, SerializedRequest>,
}
impl RequestStore {
    pub fn insert(&mut self, request: SerializedRequest) -> RequestHandle {
        self.next = self
            .next
            .checked_add(1)
            .expect("request identity exhausted");
        let id = RequestHandle(self.next);
        self.data.insert(id, request);
        id
    }
    pub fn get(&self, id: RequestHandle) -> Option<&SerializedRequest> {
        self.data.get(&id)
    }
    pub fn get_mut(&mut self, id: RequestHandle) -> Option<&mut SerializedRequest> {
        self.data.get_mut(&id)
    }
    pub fn remove(&mut self, id: RequestHandle) -> Option<SerializedRequest> {
        self.data.remove(&id)
    }
}
