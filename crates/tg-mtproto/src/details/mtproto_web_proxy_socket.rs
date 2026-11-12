//! WEB stream adapter. Browser transport is an explicit dependency of the caller.
use super::mtproto_abstract_socket::SocketEvent as Event;
use std::collections::VecDeque;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    NotConnected,
    Connecting,
    Connected,
    Disconnected,
    Error,
}
pub trait WebTransport {
    fn register_stream(&mut self, id: u32);
    fn close_stream(&mut self, id: u32);
    fn send_data(&mut self, id: u32, data: Vec<u8>);
    fn grant_window(&mut self, id: u32, size: usize);
}
pub struct WebProxySocket<T: WebTransport> {
    pub stream_id: u32,
    transport: T,
    buffer: Vec<u8>,
    offset: usize,
    pub state: State,
    pub events: VecDeque<Event>,
}
impl<T: WebTransport> WebProxySocket<T> {
    pub fn new(stream_id: u32, transport: T) -> Self {
        Self {
            stream_id,
            transport,
            buffer: Vec::new(),
            offset: 0,
            state: State::NotConnected,
            events: VecDeque::new(),
        }
    }
    pub fn connect_to_host(&mut self) {
        assert_eq!(self.state, State::NotConnected);
        self.state = State::Connecting;
        self.transport.register_stream(self.stream_id);
    }
    pub fn transport_connected(&mut self) {
        if self.state != State::Connecting {
            return;
        }
        self.state = State::Connected;
        self.events.push_back(Event::Connected);
    }
    pub fn transport_data(&mut self, data: &[u8]) {
        if self.state != State::Connected || data.is_empty() {
            return;
        }
        self.buffer.extend_from_slice(data);
        self.events.push_back(Event::ReadyRead);
    }
    pub fn transport_disconnected(&mut self) {
        if matches!(self.state, State::Disconnected | State::Error) {
            return;
        }
        self.state = State::Disconnected;
        self.events.push_back(Event::Disconnected);
    }
    pub fn transport_failed(&mut self) {
        if matches!(self.state, State::Disconnected | State::Error) {
            return;
        }
        self.state = State::Error;
        self.events.push_back(Event::Error);
    }
    pub fn has_bytes_available(&self) -> bool {
        self.offset < self.buffer.len()
    }
    pub fn read(&mut self, to: &mut [u8]) -> usize {
        let size = to.len().min(self.buffer.len() - self.offset);
        if size == 0 {
            return 0;
        }
        to[..size].copy_from_slice(&self.buffer[self.offset..self.offset + size]);
        self.offset += size;
        if self.offset == self.buffer.len() {
            self.buffer.clear();
            self.offset = 0;
        } else if self.offset >= 64 * 1024 && self.offset >= self.buffer.len() / 2 {
            self.buffer.drain(..self.offset);
            self.offset = 0;
        }
        self.transport.grant_window(self.stream_id, size);
        size
    }
    pub fn write(&mut self, prefix: &[u8], buffer: &[u8]) {
        assert!(!buffer.is_empty());
        if self.state != State::Connected {
            return;
        }
        let mut data = prefix.to_vec();
        data.extend_from_slice(buffer);
        self.transport.send_data(self.stream_id, data);
    }
}
impl<T: WebTransport> Drop for WebProxySocket<T> {
    fn drop(&mut self) {
        self.transport.close_stream(self.stream_id);
    }
}
pub use crate::connection_tcp::is_good_start_nonce;
