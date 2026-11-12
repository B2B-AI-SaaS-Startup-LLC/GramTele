//! QObject signals become explicit connection Events; ownership cancels I/O.
use tokio::io::{AsyncRead, AsyncWrite};
pub trait ByteStream: AsyncRead + AsyncWrite + Unpin {}
impl<T: AsyncRead + AsyncWrite + Unpin> ByteStream for T {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SocketKind {
    Tcp,
    FakeTls,
    Web,
}
pub fn socket_kind(secret: &[u8], web: bool) -> SocketKind {
    if web {
        SocketKind::Web
    } else if secret.len() >= 21 && secret[0] == 0xee {
        SocketKind::FakeTls
    } else {
        SocketKind::Tcp
    }
}
pub const FILES_SEND_BUFFER_SIZE: u32 = 2 * 1024 * 1024;
pub const FILES_RECEIVE_BUFFER_SIZE: u32 = 2 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SocketEvent {
    Connected,
    Disconnected,
    ReadyRead,
    Error,
    SyncTimeRequest,
}
