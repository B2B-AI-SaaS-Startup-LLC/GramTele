//! Lifecycle used by SessionPrivate: stop exchange before installing the binder.
use super::{
    mtproto_dc_key_binder::{DcKeyBindState, DcKeyBinder},
    mtproto_dc_key_creator::{DcKeyCreator, DcKeyRequest},
    mtproto_serialized_request::SerializedRequest,
};
use crate::{core_types::Random, mtproto_auth_key::AuthKey};
use std::sync::Arc;
pub struct BoundKeyCreator {
    request: DcKeyRequest,
    pub creator: Option<DcKeyCreator>,
    binder: Option<DcKeyBinder>,
}
impl BoundKeyCreator {
    pub fn new(request: DcKeyRequest) -> Self {
        Self {
            request,
            creator: None,
            binder: None,
        }
    }
    pub fn start(
        &mut self,
        dc_id: i32,
        protocol_dc_id: i16,
        random: &mut impl Random,
    ) -> Result<tg_tl::Buffer, super::mtproto_dc_key_creator::DcKeyError> {
        assert!(self.creator.is_none());
        let mut creator = DcKeyCreator::new(dc_id, protocol_dc_id, self.request);
        let request = creator.start(random)?;
        self.creator = Some(creator);
        Ok(request)
    }
    pub fn stop(&mut self) {
        self.creator = None;
    }
    pub fn bind(&mut self, key: Arc<AuthKey>) {
        self.stop();
        self.binder = Some(DcKeyBinder::new(key));
    }
    pub fn restart_binder(&mut self) {
        if let Some(binder) = &self.binder {
            self.binder = Some(DcKeyBinder::new(binder.persistent_key()));
        }
    }
    pub fn ready_to_bind(&self) -> bool {
        self.binder.is_some()
    }
    pub fn prepare_bind_request(
        &self,
        key: &AuthKey,
        session_id: u64,
        msg_id: u64,
        now_ms: i64,
        random: &mut impl Random,
    ) -> SerializedRequest {
        self.binder
            .as_ref()
            .expect("binder ready")
            .prepare_request(key, session_id, msg_id, now_ms, random)
    }
    pub fn handle_bind_response(&self, response: &[i32]) -> DcKeyBindState {
        self.binder
            .as_ref()
            .expect("binder ready")
            .handle_response(response)
    }
    pub fn bind_persistent_key(&self) -> Arc<AuthKey> {
        self.binder.as_ref().expect("binder ready").persistent_key()
    }
}
pub fn is_destroyed_temporary_key_error(response: &[i32]) -> bool {
    let mut reader = tg_tl::Reader::new(response);
    <tg_scheme::RpcError as tg_tl::Deserialize>::read(&mut reader).is_ok_and(|e| {
        let e = e.c_rpc_error();
        e.error_code == 401 && e.error_message == b"AUTH_KEY_PERM_EMPTY"
    })
}
