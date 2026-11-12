//! SessionPrivate protocol state machine, encrypted envelopes and virtual timers.
//! The environment and transport executors are supplied by the caller.
use crate::{
    connection_abstract::TransportError,
    core_types::Buffer,
    mtproto_auth_key::{aes_ige_decrypt_raw, aes_ige_encrypt_raw, AuthKey},
};
use tg_base::openssl_help::{const_time_equal, sha256_parts};
use zeroize::Zeroizing;
pub const MAX_MESSAGE_LENGTH: usize = 16 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    ClientToServer,
    ServerToClient,
}
impl Direction {
    fn send(self) -> bool {
        self == Self::ClientToServer
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct Message {
    pub salt: u64,
    pub session_id: u64,
    pub msg_id: u64,
    pub seq_no: u32,
    pub body: Buffer,
}
impl Message {
    pub fn needs_ack(&self) -> bool {
        self.seq_no & 1 != 0
    }
}
/// Encrypt an envelope with explicitly supplied padding. Packet includes the two
/// reserved transport words consumed by both TcpConnection and HttpConnection.
pub fn encrypt_message(
    key: &AuthKey,
    message: &Message,
    padding: &[u8],
    direction: Direction,
) -> Result<Buffer, TransportError> {
    let length = message
        .body
        .len()
        .checked_mul(4)
        .ok_or(TransportError::Protocol("bad outgoing message length"))?;
    if message.body.is_empty()
        || !(12..=1024).contains(&padding.len())
        || length > MAX_MESSAGE_LENGTH
        || !(32 + length + padding.len()).is_multiple_of(16)
    {
        return Err(TransportError::Protocol(
            "bad outgoing message length or padding",
        ));
    }
    let mut plain = Zeroizing::new(Vec::with_capacity(32 + length + padding.len()));
    plain.extend_from_slice(&message.salt.to_le_bytes());
    plain.extend_from_slice(&message.session_id.to_le_bytes());
    plain.extend_from_slice(&message.msg_id.to_le_bytes());
    plain.extend_from_slice(&message.seq_no.to_le_bytes());
    plain.extend_from_slice(&(length as u32).to_le_bytes());
    plain.extend_from_slice(&tg_tl::primes_to_bytes(&message.body));
    plain.extend_from_slice(padding);
    let hash = sha256_parts(&[key.part_for_msg_key(direction.send()), &plain]);
    let msg_key: [u8; 16] = hash[8..24].try_into().unwrap();
    let (aes, iv) = key.prepare_aes(&msg_key, direction.send());
    aes_ige_encrypt_raw(&mut plain, &aes, &iv).map_err(TransportError::Protocol)?;
    let mut result = vec![0, 0];
    result.extend(tg_tl::bytes_to_primes(&key.key_id().to_le_bytes()));
    result.extend(tg_tl::bytes_to_primes(&msg_key));
    result.extend(tg_tl::bytes_to_primes(&plain));
    Ok(result)
}
#[derive(Debug, PartialEq, Eq)]
pub struct ReceivedMessage {
    pub message: Message,
    pub bad_time: bool,
    pub padding_len: usize,
}
/// Packet excludes the reserved transport words. Extra unencrypted padding of
/// 0..3 words is ignored just as in original padded-intermediate transport.
pub fn decrypt_message(
    key: &AuthKey,
    packet: &[i32],
    session_id: u64,
    unix_seconds: i64,
) -> Result<ReceivedMessage, TransportError> {
    if packet.len() < 18 || packet.len() > MAX_MESSAGE_LENGTH / 4 {
        return Err(TransportError::Protocol("bad message received length"));
    }
    let bytes = tg_tl::primes_to_bytes(packet);
    if u64::from_le_bytes(bytes[..8].try_into().unwrap()) != key.key_id() {
        return Err(TransportError::Protocol("bad auth_key_id"));
    }
    let msg_key: [u8; 16] = bytes[8..24].try_into().unwrap();
    let count = (packet.len() - 6) & !3;
    let mut plain = Zeroizing::new(bytes[24..24 + count * 4].to_vec());
    let (aes, iv) = key.prepare_aes(&msg_key, false);
    aes_ige_decrypt_raw(&mut plain, &aes, &iv).map_err(TransportError::Protocol)?;
    let hash = sha256_parts(&[key.part_for_msg_key(false), &plain]);
    if !const_time_equal(&msg_key, &hash[8..24]) {
        return Err(TransportError::Protocol("bad SHA256 hash after aesDecrypt"));
    }
    let length = u32::from_le_bytes(plain[28..32].try_into().unwrap()) as usize;
    let padding_len = plain.len().wrapping_sub(32 + length);
    if length > MAX_MESSAGE_LENGTH
        || !length.is_multiple_of(4)
        || !(12..=1024).contains(&padding_len)
    {
        return Err(TransportError::Protocol("bad msg_len received"));
    }
    let read64 = |offset| u64::from_le_bytes(plain[offset..offset + 8].try_into().unwrap());
    if read64(8) != session_id {
        return Err(TransportError::Protocol("bad server session received"));
    }
    let msg_id = read64(16);
    if !matches!(msg_id & 3, 1 | 3) {
        return Err(TransportError::Protocol("bad msg_id in message received"));
    }
    let server_time = (msg_id >> 32) as i32 as i64;
    let bad_time = server_time > unix_seconds + 60 || server_time + 300 < unix_seconds;
    Ok(ReceivedMessage {
        message: Message {
            salt: read64(0),
            session_id,
            msg_id,
            seq_no: u32::from_le_bytes(plain[24..28].try_into().unwrap()),
            body: tg_tl::bytes_to_primes(&plain[32..32 + length]),
        },
        padding_len,
        bad_time,
    })
}

mod machine;
pub use machine::{
    compute_app_version, log_ids_vector, register_options, AppDistribution, ClientInfo,
    ConnectionSpec, HandleResult, SessionClock, SessionPrivate, TransportAction, CONNECTED,
    CONNECTING, DISCONNECTED, OPTION_PREFER_IPV6,
};
