//! Account map handles and MTProto authorization only. Cache/settings are not loaded.
use super::details::storage_file_utilities::*;
use crate::main::main_account::Authorization;
use md5::{Digest, Md5};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tg_base::qt_stream::Reader;
use tg_mtproto::{core_types::Random, mtproto_auth_key::AuthKey, mtproto_config::Config};
pub fn compute_data_name_key(name: &str) -> u64 {
    u64::from_le_bytes(Md5::digest(name.as_bytes())[..8].try_into().unwrap())
}
/// Raw known map records; payloads retained without opening settings/chat/cache files.
#[derive(Default, Clone)]
pub struct AccountMap {
    pub data: Vec<u8>,
}
impl AccountMap {
    pub fn parse(data: Vec<u8>) -> Result<Self> {
        let mut r = Reader::new(&data);
        while !r.at_end() {
            let tag = check_stream_status(r.i32())?;
            match tag {
                1 | 2 | 0x1d => {
                    let count = check_stream_status(r.i32())?;
                    if count < 0 {
                        return Err(Error::Invalid("bad map count"));
                    }
                    check_stream_status(
                        r.raw(
                            (count as usize)
                                .checked_mul(16)
                                .ok_or(Error::Invalid("map count overflow"))?,
                        ),
                    )?;
                }
                3 | 5 | 6 => {
                    let count = check_stream_status(r.i32())?;
                    if count < 0 {
                        return Err(Error::Invalid("bad legacy map count"));
                    }
                    check_stream_status(
                        r.raw(
                            (count as usize)
                                .checked_mul(28)
                                .ok_or(Error::Invalid("map count overflow"))?,
                        ),
                    )?;
                }
                0x15 => {
                    check_stream_status(r.bytes())?;
                }
                0x19 => {
                    check_stream_status(r.bytes())?;
                    check_stream_status(r.bytes())?;
                }
                0x10 => {
                    check_stream_status(r.raw(32))?;
                }
                0x14 => {
                    check_stream_status(r.raw(16))?;
                }
                0x16 | 0x17 => {
                    check_stream_status(r.raw(24))?;
                }
                4 | 7..=0x0f | 0x11..=0x13 | 0x18 | 0x1a..=0x1c | 0x1e => {
                    check_stream_status(r.raw(8))?;
                }
                _ => return Err(Error::Invalid("unknown key type in encrypted map")),
            }
        }
        Ok(Self { data })
    }
}
pub struct Account {
    pub root: PathBuf,
    pub data_name: String,
    pub data_name_key: u64,
    pub base_path: PathBuf,
    pub local_key: Option<Arc<AuthKey>>,
    pub map: AccountMap,
    pub authorization: Authorization,
    pub old_map_version: i32,
}
impl Account {
    pub fn new(root: &Path, data_name: &str) -> Self {
        let key = compute_data_name_key(data_name);
        Self {
            root: root.into(),
            data_name: data_name.into(),
            data_name_key: key,
            base_path: root.join(to_file_part(key)),
            local_key: None,
            map: AccountMap::default(),
            authorization: Authorization::default(),
            old_map_version: 0,
        }
    }
    pub fn temp_directory(&self) -> PathBuf {
        self.root.join(format!("temp_{}", self.data_name))
    }
    pub fn support_mode_path(&self) -> PathBuf {
        self.root
            .join(format!("user_{}", self.data_name))
            .join("support")
    }
    pub fn start_added(&mut self, key: Arc<AuthKey>) {
        self.local_key = Some(key);
    }
    pub fn start(&mut self, key: Arc<AuthKey>) -> Result<Option<Config>> {
        self.local_key = Some(key);
        if self.read_map_with(None, &[]).is_ok() {
            let _ = self.read_mtp_data();
        }
        Ok(self.read_mtp_config().ok().flatten())
    }
    pub fn legacy_start(&mut self, passcode: &[u8]) -> Result<()> {
        match self.read_map_with(None, passcode) {
            Ok(()) => {
                let _ = self.read_mtp_data();
                Ok(())
            }
            Err(Error::IncorrectPasscode) => Err(Error::IncorrectPasscodeLegacy),
            Err(_) => {
                assert!(self.local_key.is_none());
                Ok(())
            }
        }
    }
    pub fn read_map_with(&mut self, key: Option<Arc<AuthKey>>, passcode: &[u8]) -> Result<()> {
        let file = read_file("map", &self.base_path)?;
        let mut r = file.reader();
        let salt = check_stream_status(r.bytes())?;
        let key_encrypted = check_stream_status(r.bytes())?;
        let map_encrypted = check_stream_status(r.bytes())?;
        let key = match key.or_else(|| self.local_key.clone()) {
            Some(key) => key,
            None => {
                if salt.len() != SALT_SIZE {
                    return Err(Error::Invalid("bad salt in map file"));
                }
                let inner =
                    decrypt_local(&key_encrypted, &create_legacy_local_key(passcode, &salt))
                        .map_err(|_| Error::IncorrectPasscode)?;
                let raw = inner
                    .stream
                    .0
                    .get(4..)
                    .ok_or(Error::Invalid("bad legacy local key"))?;
                let data = raw
                    .try_into()
                    .map_err(|_| Error::Invalid("bad legacy local key size"))?;
                Arc::new(AuthKey::local(data))
            }
        };
        let inner = decrypt_local(&map_encrypted, &key)
            .map_err(|_| Error::Invalid("could not decrypt map"))?;
        self.map = AccountMap::parse(inner.stream.0[4..].to_vec())?;
        self.local_key = Some(key);
        self.old_map_version = file.version;
        Ok(())
    }
    pub fn write_map(&self, manager: &mut WriteManager, random: &mut impl Random) -> Result<()> {
        let key = self
            .local_key
            .as_ref()
            .ok_or(Error::Invalid("local key missing"))?;
        let mut file = FileWriteDescriptor::new("map", &self.base_path);
        file.write_data(None);
        file.write_data(None);
        let mut inner = EncryptedDescriptor::new(self.map.data.len());
        inner.stream.0.extend(&self.map.data);
        file.write_encrypted(&mut inner, key, random);
        file.finish(manager, false)
    }
    pub fn write_mtp_data(
        &self,
        manager: &mut WriteManager,
        random: &mut impl Random,
    ) -> Result<()> {
        let key = self
            .local_key
            .as_ref()
            .ok_or(Error::Invalid("local key missing"))?;
        let mut file = FileWriteDescriptor::new(&to_file_part(self.data_name_key), &self.root);
        let mut inner = EncryptedDescriptor::new(0);
        inner.stream.i32(0x4b);
        inner
            .stream
            .bytes(&self.authorization.serialize_mtp_authorization());
        file.write_encrypted(&mut inner, key, random);
        file.finish(manager, false)
    }
    pub fn read_mtp_data(&mut self) -> Result<()> {
        let key = self
            .local_key
            .as_ref()
            .ok_or(Error::Invalid("local key missing"))?;
        let file = read_encrypted_file(&to_file_part(self.data_name_key), &self.root, key)?;
        let mut r = file.reader();
        let mut modern = Vec::new();
        let mut legacy = Authorization::default();
        while !r.at_end() {
            match check_stream_status(r.i32())? {
                0x4b => modern = check_stream_status(r.bytes())?,
                0 => {
                    let dc = check_stream_status(r.i32())?;
                    let data = check_stream_status(r.raw(256))?.try_into().unwrap();
                    legacy.keys.push(Arc::new(AuthKey::new(
                        tg_mtproto::mtproto_auth_key::KeyType::ReadFromFile,
                        dc,
                        data,
                        0,
                    )));
                }
                1 => {
                    legacy.user_id = check_stream_status(r.i32())? as u64;
                    legacy.main_dc_id = check_stream_status(r.i32())?;
                }
                _ => return Err(Error::Invalid("legacy ReadSetting block deferred")),
            }
        }
        self.apply_read_authorization_context(&modern, legacy)?;
        Ok(())
    }
    pub fn apply_read_authorization_context(
        &mut self,
        modern: &[u8],
        legacy: Authorization,
    ) -> Result<()> {
        if !modern.is_empty() {
            self.authorization.set_mtp_authorization(modern)
        } else {
            self.authorization.keys.extend(legacy.keys);
            if legacy.main_dc_id != 0 {
                self.authorization.main_dc_id = legacy.main_dc_id;
                self.authorization.user_id = legacy.user_id;
            }
            Ok(())
        }
    }
    pub fn write_mtp_config(
        &self,
        config: &Config,
        manager: &mut WriteManager,
        random: &mut impl Random,
    ) -> Result<()> {
        let key = self
            .local_key
            .as_ref()
            .ok_or(Error::Invalid("local key missing"))?;
        let mut inner = EncryptedDescriptor::new(0);
        inner.stream.bytes(&config.serialize());
        let mut file = FileWriteDescriptor::new("config", &self.base_path);
        file.write_encrypted(&mut inner, key, random);
        file.finish(manager, false)
    }
    pub fn read_mtp_config(&self) -> Result<Option<Config>> {
        let file = read_encrypted_file(
            "config",
            &self.base_path,
            self.local_key
                .as_ref()
                .ok_or(Error::Invalid("local key missing"))?,
        )?;
        let mut r = file.reader();
        Ok(Config::from_serialized(&check_stream_status(r.bytes())?))
    }
}

/// Non-UI map scheduling. The caller supplies the same TimerHost as its main queue.
pub struct AccountWrites {
    timer: tg_base::timer::Timer,
    due: std::sync::Arc<std::sync::atomic::AtomicBool>,
    changed: bool,
}
impl AccountWrites {
    pub fn new(host: &std::rc::Rc<tg_base::timer::TimerHost>) -> Self {
        let due = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fire = due.clone();
        Self {
            timer: tg_base::timer::Timer::in_host(
                host,
                Some(std::rc::Rc::new(move || {
                    fire.store(true, std::sync::atomic::Ordering::Release)
                })),
            ),
            due,
            changed: false,
        }
    }
    pub fn write_map_delayed(&mut self) {
        self.changed = true;
        self.timer.call_once(1000);
    }
    pub fn write_map_queued(&mut self) {
        self.changed = true;
        let due = self.due.clone();
        tg_runtime::on_main(move || due.store(true, std::sync::atomic::Ordering::Release));
    }
    pub fn process(
        &mut self,
        account: &Account,
        manager: &mut WriteManager,
        random: &mut impl Random,
    ) -> Result<bool> {
        if !self.due.swap(false, std::sync::atomic::Ordering::AcqRel) {
            return Ok(false);
        }
        self.timer.cancel();
        if !self.changed {
            return Ok(false);
        }
        self.changed = false;
        account.write_map(manager, random)?;
        Ok(true)
    }
}
