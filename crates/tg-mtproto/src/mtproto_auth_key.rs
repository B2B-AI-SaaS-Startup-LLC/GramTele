//! `mtproto_auth_key.h/.cpp`. MTProto v2 and legacy local/bind key schedules.
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, StreamCipher};
use tg_base::openssl_help::{sha1, sha1_parts, sha256_parts};
use zeroize::Zeroize;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyType {
    Generated,
    Temporary,
    ReadFromFile,
    Local,
}
pub struct AuthKey {
    kind: KeyType,
    dc_id: i32,
    data: [u8; 256],
    key_id: u64,
    creation_time: i64,
    expires_at: i64,
}
impl Drop for AuthKey {
    fn drop(&mut self) {
        self.data.zeroize();
    }
}
impl AuthKey {
    pub fn new(kind: KeyType, dc_id: i32, data: [u8; 256], now_ms: i64) -> Self {
        let hash = sha1(&data);
        Self {
            kind,
            dc_id,
            data,
            key_id: u64::from_le_bytes(hash[12..20].try_into().unwrap()),
            creation_time: if matches!(kind, KeyType::Generated | KeyType::Temporary) {
                now_ms
            } else {
                0
            },
            expires_at: 0,
        }
    }
    pub fn local(data: [u8; 256]) -> Self {
        Self::new(KeyType::Local, 0, data, 0)
    }
    pub fn kind(&self) -> KeyType {
        self.kind
    }
    pub fn dc_id(&self) -> i32 {
        self.dc_id
    }
    pub fn key_id(&self) -> u64 {
        self.key_id
    }
    pub fn creation_time(&self) -> i64 {
        self.creation_time
    }
    pub fn expires_at(&self) -> i64 {
        self.expires_at
    }
    pub fn set_expires_at(&mut self, time: i64) {
        assert_eq!(self.kind, KeyType::Temporary);
        self.expires_at = time;
    }
    pub fn data(&self) -> &[u8; 256] {
        &self.data
    }
    pub fn equals(&self, other: &Self) -> bool {
        self.data == other.data
    }
    pub fn write(&self, to: &mut Vec<u8>) {
        to.extend_from_slice(&self.data);
    }
    pub fn fill_data(computed: &[u8]) -> [u8; 256] {
        assert!(computed.len() <= 256);
        let mut result = [0; 256];
        result[256 - computed.len()..].copy_from_slice(computed);
        result
    }
    pub fn part_for_msg_key(&self, send: bool) -> &[u8] {
        let x = if send { 0 } else { 8 };
        &self.data[88 + x..120 + x]
    }
    pub fn prepare_aes(&self, msg_key: &[u8; 16], send: bool) -> ([u8; 32], [u8; 32]) {
        let x = if send { 0 } else { 8 };
        let a = sha256_parts(&[msg_key.as_slice(), &self.data[x..x + 36]]);
        let b = sha256_parts(&[&self.data[40 + x..76 + x], msg_key.as_slice()]);
        (
            join32(&[&a[..8], &b[8..24], &a[24..32]]),
            join32(&[&b[..8], &a[8..24], &b[24..32]]),
        )
    }
    pub fn prepare_aes_oldmtp(&self, msg_key: &[u8; 16], send: bool) -> ([u8; 32], [u8; 32]) {
        let x = if send { 0 } else { 8 };
        let k = &self.data;
        let a = sha1_parts(&[msg_key.as_slice(), &k[x..x + 32]]);
        let b = sha1_parts(&[&k[32 + x..48 + x], msg_key.as_slice(), &k[48 + x..64 + x]]);
        let c = sha1_parts(&[&k[64 + x..96 + x], msg_key.as_slice()]);
        let d = sha1_parts(&[msg_key.as_slice(), &k[96 + x..128 + x]]);
        (
            join32(&[&a[..8], &b[8..20], &c[4..16]]),
            join32(&[&a[8..20], &b[..8], &c[16..20], &d[..8]]),
        )
    }
}
fn join32(parts: &[&[u8]]) -> [u8; 32] {
    parts.concat().try_into().unwrap()
}
pub fn aes_ige_encrypt_raw(
    data: &mut [u8],
    key: &[u8; 32],
    iv: &[u8; 32],
) -> Result<(), &'static str> {
    if !data.len().is_multiple_of(16) {
        return Err("AES-IGE data length is not a multiple of 16");
    }
    let mut cipher = ige::Encryptor::<aes::Aes256>::new(key.into(), iv.into());
    for block in data.chunks_exact_mut(16) {
        cipher.encrypt_block(block.try_into().unwrap());
    }
    Ok(())
}
pub fn aes_ige_decrypt_raw(
    data: &mut [u8],
    key: &[u8; 32],
    iv: &[u8; 32],
) -> Result<(), &'static str> {
    if !data.len().is_multiple_of(16) {
        return Err("AES-IGE data length is not a multiple of 16");
    }
    let mut cipher = ige::Decryptor::<aes::Aes256>::new(key.into(), iv.into());
    for block in data.chunks_exact_mut(16) {
        cipher.decrypt_block(block.try_into().unwrap());
    }
    Ok(())
}
/// OpenSSL CTR carries the partial block and increments its 128-bit counter BE.
pub struct CtrState(ctr::Ctr128BE<aes::Aes256>);
impl CtrState {
    pub fn new(key: &[u8; 32], iv: &[u8; 16]) -> Self {
        Self(ctr::Ctr128BE::new(key.into(), iv.into()))
    }
    pub fn apply(&mut self, data: &mut [u8]) {
        self.0.apply_keystream(data);
    }
}
