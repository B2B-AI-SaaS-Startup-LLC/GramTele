//! bindTempAuthKey uses the original legacy v1 schedule inside a v2 session request.
use super::mtproto_serialized_request::SerializedRequest;
use crate::{
    core_types::Random,
    mtproto_auth_key::{aes_ige_encrypt_raw, AuthKey},
};
use std::sync::Arc;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DcKeyBindState {
    Success,
    Failed,
    DefinitelyDestroyed,
}
pub struct DcKeyBinder {
    persistent_key: Arc<AuthKey>,
}
impl DcKeyBinder {
    pub fn new(persistent_key: Arc<AuthKey>) -> Self {
        Self { persistent_key }
    }
    pub fn persistent_key(&self) -> Arc<AuthKey> {
        self.persistent_key.clone()
    }
    pub fn prepare_request(
        &self,
        temporary_key: &AuthKey,
        session_id: u64,
        msg_id: u64,
        now_ms: i64,
        random: &mut impl Random,
    ) -> SerializedRequest {
        assert_ne!(temporary_key.expires_at(), 0);
        let nonce = random.u64();
        let inner = tg_scheme::mtp_bind_auth_key_inner(
            nonce,
            temporary_key.key_id(),
            self.persistent_key.key_id(),
            session_id,
            temporary_key.expires_at() as i32,
        );
        let mut serialized = SerializedRequest::serialize(&inner, now_ms);
        serialized.set_msg_id(msg_id);
        serialized.set_seq_no(0);
        serialized.add_padding(true, random);
        for word in &mut serialized.words[..4] {
            *word = random.u32() as i32;
        }
        let mut bytes = tg_tl::primes_to_bytes(&serialized.words);
        let hash = tg_base::openssl_help::sha1(&bytes[..(4 + serialized.message_size()) * 4]);
        let msg_key = hash[4..20].try_into().unwrap();
        let (key, iv) = self.persistent_key.prepare_aes_oldmtp(&msg_key, true);
        aes_ige_encrypt_raw(&mut bytes, &key, &iv).expect("aligned binding plaintext");
        let mut encrypted = self.persistent_key.key_id().to_le_bytes().to_vec();
        encrypted.extend(msg_key);
        encrypted.extend(bytes);
        let mut request = SerializedRequest::serialize(
            &tg_scheme::AuthBindTempAuthKey::new(
                self.persistent_key.key_id(),
                nonce,
                temporary_key.expires_at() as i32,
                encrypted,
            ),
            now_ms,
        );
        request.set_msg_id(msg_id);
        request
    }
    pub fn handle_response(&self, response: &[i32]) -> DcKeyBindState {
        if response.first().copied().map(|v| v as u32) == Some(tg_tl::ID_BOOL_TRUE) {
            return DcKeyBindState::Success;
        }
        let mut reader = tg_tl::Reader::new(response);
        if let Ok(error) = <tg_scheme::RpcError as tg_tl::Deserialize>::read(&mut reader) {
            let error = error.c_rpc_error();
            if error.error_code == 400 && error.error_message == b"ENCRYPTED_MESSAGE_INVALID" {
                return DcKeyBindState::DefinitelyDestroyed;
            }
        }
        DcKeyBindState::Failed
    }
}
