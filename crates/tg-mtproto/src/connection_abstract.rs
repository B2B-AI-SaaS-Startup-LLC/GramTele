//! Session-facing event/packet contract. Packet words have two reserved TCP words.
use crate::core_types::{Buffer, Random, Time};
use std::collections::VecDeque;
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Event {
    ReceivedData,
    ReceivedSome,
    Connected,
    Disconnected,
    Error(i32),
    SyncTimeRequest,
}
#[derive(Debug)]
pub enum TransportError {
    Io(std::io::Error),
    Http(reqwest::Error),
    Protocol(&'static str),
    Server(i32),
}
impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Http(e) => write!(f, "{e}"),
            Self::Protocol(e) => f.write_str(e),
            Self::Server(code) => write!(f, "server error {code}"),
        }
    }
}
impl std::error::Error for TransportError {}
impl From<std::io::Error> for TransportError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<reqwest::Error> for TransportError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e)
    }
}
/// The owner supplies monotonic and synchronized Unix time; tests can change both.
pub trait Clock {
    fn now(&self) -> Time;
    fn update_unix_time(&self, server_time: i32);
    fn next_message_id(&self) -> u64;
}
/// Network-independent exchange contract used by key creation; implement in memory.
#[allow(async_fn_in_trait)]
pub trait Exchange {
    async fn exchange(&mut self, packet: Buffer) -> Result<Buffer, TransportError>;
}
#[derive(Default)]
pub struct ConnectionState {
    pub received: VecDeque<Buffer>,
    pub events: VecDeque<Event>,
    pub sent_encrypted_with_key_id: u64,
    pub connected: bool,
    pub ping_time: i64,
}
impl ConnectionState {
    pub fn push_received(&mut self, buffer: Buffer) {
        self.received.push_back(buffer);
        self.events.push_back(Event::ReceivedData);
    }
    pub fn disconnect(&mut self) {
        self.connected = false;
        self.events.push_back(Event::Disconnected);
    }
}
pub fn prepare_not_secure_packet<T: tg_tl::Serialize>(
    request: &T,
    id: u64,
    random: &mut impl Random,
) -> Buffer {
    let body = tg_tl::serialize(request);
    let mut b = vec![0, 0, 0, 0, id as i32, (id >> 32) as i32, 0];
    b.extend(body);
    let mut byte = [0];
    random.fill(&mut byte);
    let padding = (byte[0] & 63) as usize;
    b[6] = ((b.len() - 7 + padding) * 4) as i32;
    for _ in 0..padding {
        b.push(random.u32() as i32);
    }
    b
}
pub fn prepare_secure_packet(key_id: u64, msg_key: &[u8; 16], size: usize) -> Buffer {
    let mut result = Vec::with_capacity(8 + size + 4);
    result.extend([0, 0, key_id as i32, (key_id >> 32) as i32]);
    result.extend(tg_tl::bytes_to_primes(msg_key));
    result
}
pub fn parse_not_secure_response(buffer: &[i32]) -> Result<&[i32], TransportError> {
    if buffer.len() < 6 {
        return Err(TransportError::Protocol("bad unencrypted response length"));
    }
    if buffer[0] != 0 || buffer[1] != 0 || buffer[2] as u32 & 3 != 1 {
        return Err(TransportError::Protocol("bad unencrypted response header"));
    }
    let size = buffer[4] as u32 as usize;
    if size < 1 || !size.is_multiple_of(4) || size > (buffer.len() - 5) * 4 {
        return Err(TransportError::Protocol(
            "bad unencrypted response body length",
        ));
    }
    Ok(&buffer[5..5 + size / 4])
}
pub fn protocol_dc_debug_id(id: i16) -> String {
    let id = id as i32;
    let abs = id.abs();
    format!(
        "{}{}{}",
        if abs > 10000 { "test_" } else { "" },
        if abs > 10000 { abs - 10000 } else { abs },
        if id < 0 { "_media" } else { "" }
    )
}

/// Ready adapter for the already-ported runtime and server time adjustment.
pub struct RuntimeClock;
impl Clock for RuntimeClock {
    fn next_message_id(&self) -> u64 {
        tg_base::unixtime::mtproto_msg_id()
    }
    fn now(&self) -> Time {
        Time {
            monotonic_ms: tg_runtime::now(),
            unix_seconds: tg_base::unixtime::now() as i64,
        }
    }
    fn update_unix_time(&self, server_time: i32) {
        tg_base::unixtime::update(server_time, false);
    }
}
impl ConnectionState {
    pub fn dispatch_events_on_main(&mut self, callback: impl FnOnce(Vec<Event>) + Send + 'static) {
        let events = self.events.drain(..).collect();
        tg_runtime::on_main(move || callback(events));
    }
}
