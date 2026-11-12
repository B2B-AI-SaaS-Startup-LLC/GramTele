//! Authorization record, independent of the future Main::Session/UI.
use crate::storage::details::storage_file_utilities::{check_stream_status as get, Error, Result};
use std::sync::Arc;
use tg_base::qt_stream::{Reader, Writer};
use tg_mtproto::{
    mtp_instance::{Fields, Instance},
    mtproto_auth_key::{AuthKey, KeyType},
    mtproto_dc_options::Environment,
};
pub fn compose_data_string(name: &str, index: i32) -> String {
    let mut result = name.replace('#', "");
    if index > 0 {
        result.push_str(&format!("#{}", index + 1));
    }
    result
}
#[derive(Clone, Default)]
pub struct Authorization {
    pub user_id: u64,
    pub main_dc_id: i32,
    pub keys: Vec<Arc<AuthKey>>,
    pub keys_to_destroy: Vec<Arc<AuthKey>>,
}
impl Authorization {
    pub fn serialize_mtp_authorization(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u64(u64::MAX);
        w.u64(self.user_id);
        w.i32(self.main_dc_id);
        for list in [&self.keys, &self.keys_to_destroy] {
            w.i32(list.len() as i32);
            for key in list {
                w.i32(key.dc_id());
                w.0.extend(key.data());
            }
        }
        w.0
    }
    /// Like the original, already read fields/keys remain assigned on truncation.
    pub fn set_mtp_authorization(&mut self, bytes: &[u8]) -> Result<()> {
        let mut r = Reader::new(bytes);
        let old_user = get(r.i32())?;
        let old_dc = get(r.i32())?;
        let wide = ((old_user as u64) << 32) | (old_dc as u64);
        let (user, dc) = if wide == u64::MAX {
            (get(r.u64())?, get(r.i32())?)
        } else {
            (old_user as u64, old_dc)
        };
        self.user_id = user;
        self.main_dc_id = dc;
        for list in [&mut self.keys, &mut self.keys_to_destroy] {
            let count = get(r.i32())?;
            if count < 0 {
                return Err(Error::Invalid("bad authorization key count"));
            }
            for _ in 0..count {
                let dc = get(r.i32())?;
                let key = get(r.raw(256))?.try_into().unwrap();
                list.push(Arc::new(AuthKey::new(KeyType::ReadFromFile, dc, key, 0)));
            }
        }
        Ok(())
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut result = Self::default();
        result.set_mtp_authorization(bytes)?;
        Ok(result)
    }
    pub fn from_instance(user_id: u64, instance: &Instance, destroyer: Option<&Instance>) -> Self {
        Self {
            user_id,
            main_dc_id: instance.main_dc_id(),
            keys: instance.get_keys_for_write(),
            keys_to_destroy: destroyer
                .map(|i| i.get_keys_for_write())
                .unwrap_or_default(),
        }
    }
    pub fn fields(&self, environment: Environment) -> Fields {
        let mut fields = Fields::new(environment);
        fields.main_dc_id = self.main_dc_id;
        fields.keys = self.keys.clone();
        fields
    }
    pub fn apply_keys_to_destroy(&self, instance: &Instance) {
        instance.add_keys_for_destroy(self.keys_to_destroy.clone());
    }
    pub fn session_unique_id(&self, test: bool) -> u64 {
        if self.user_id == 0 {
            0
        } else {
            self.user_id | if test { 1 << 56 } else { 0 }
        }
    }
}
