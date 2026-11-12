//! key_datas passcode/local key/account-list envelope.
use super::{details::storage_file_utilities::*, storage_account::Account};
use crate::main::{
    main_account::compose_data_string,
    main_domain::{Domain as MainDomain, PREMIUM_MAX_ACCOUNTS},
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
};
use tg_base::qt_stream::Reader;
use tg_mtproto::{core_types::Random, mtproto_auth_key::AuthKey};
pub fn compute_key_name(data_name: &str) -> String {
    format!("key_{data_name}")
}
pub struct Domain {
    pub root: PathBuf,
    pub data_name: String,
    pub owner: MainDomain,
    pub local_key: Option<Arc<AuthKey>>,
    passcode_key: Option<Arc<AuthKey>>,
    pub passcode_key_salt: Vec<u8>,
    pub passcode_key_encrypted: Vec<u8>,
    old_version: i32,
    has_passcode: bool,
    changes: Vec<mpsc::Sender<()>>,
}
impl Domain {
    pub fn new(root: &Path, data_name: &str) -> Self {
        Self {
            root: root.into(),
            data_name: data_name.into(),
            owner: MainDomain::default(),
            local_key: None,
            passcode_key: None,
            passcode_key_salt: vec![],
            passcode_key_encrypted: vec![],
            old_version: 0,
            has_passcode: false,
            changes: vec![],
        }
    }
    /// Strict reader: no implicit destructive reset if a synthetic/storage input is damaged.
    pub fn start_modern(&mut self, passcode: &[u8]) -> Result<()> {
        let file = read_file(&compute_key_name(&self.data_name), &self.root)?;
        let mut r = file.reader();
        let salt = check_stream_status(r.bytes())?;
        let enc = check_stream_status(r.bytes())?;
        let info = check_stream_status(r.bytes())?;
        if salt.len() != SALT_SIZE {
            return Err(Error::Invalid("bad salt in info file"));
        }
        let passkey = create_local_key(passcode, &salt);
        let decrypted = decrypt_local(&enc, &passkey)?;
        let key: [u8; 256] = decrypted.stream.0[4..]
            .try_into()
            .map_err(|_| Error::Invalid("bad local key size"))?;
        let local = Arc::new(AuthKey::local(key));
        let decrypted = decrypt_local(&info, &local)?;
        let mut r = Reader::new(&decrypted.stream.0[4..]);
        let count = check_stream_status(r.i32())?;
        if !(1..=PREMIUM_MAX_ACCOUNTS).contains(&count) {
            return Err(Error::Invalid("bad accounts count"));
        }
        let mut tried = BTreeSet::new();
        let mut sessions = BTreeSet::new();
        let mut owner = MainDomain::default();
        let mut active = 0;
        for i in 0..count {
            let index = r.i32().unwrap_or_default();
            if (0..PREMIUM_MAX_ACCOUNTS).contains(&index) && tried.insert(index) {
                let mut account =
                    Account::new(&self.root, &compose_data_string(&self.data_name, index));
                let config = account.start(local.clone()).ok().flatten();
                let id = account
                    .authorization
                    .session_unique_id(config.is_some_and(|c| c.is_test_mode()));
                if !sessions.contains(&id) && (id != 0 || (sessions.is_empty() && i + 1 == count)) {
                    if sessions.is_empty() {
                        active = index;
                    }
                    owner.account_added_in_storage(index, account.authorization)?;
                    sessions.insert(id);
                }
            }
        }
        if sessions.is_empty() {
            return Err(Error::Invalid("no accounts read"));
        }
        if !r.at_end() {
            active = r.i32().unwrap_or_default();
        }
        owner.activate_from_storage(active);
        self.owner = owner;
        self.local_key = Some(local);
        self.passcode_key = Some(passkey);
        self.passcode_key_salt = salt;
        self.passcode_key_encrypted = enc;
        self.has_passcode = !passcode.is_empty();
        self.old_version = file.version;
        Ok(())
    }
    pub fn start(
        &mut self,
        passcode: &[u8],
        random: &mut impl Random,
        manager: &mut WriteManager,
    ) -> Result<()> {
        match self.start_modern(passcode) {
            Ok(()) => {
                if self.old_version < APP_VERSION {
                    self.write_accounts(manager, random)?;
                }
                Ok(())
            }
            Err(Error::IncorrectPasscode) => Err(Error::IncorrectPasscode),
            Err(Error::Missing) => {
                let mut account =
                    Account::new(&self.root, &compose_data_string(&self.data_name, 0));
                match account.legacy_start(passcode) {
                    Ok(()) => self.start_with_single_account(passcode, account, random, manager),
                    Err(Error::IncorrectPasscode) => Err(Error::IncorrectPasscode),
                    _ => self.start_from_scratch(random, manager),
                }
            }
            Err(_) => self.start_from_scratch(random, manager),
        }
    }
    pub fn generate_local_key(&mut self, random: &mut impl Random) -> Result<()> {
        assert!(
            self.local_key.is_none()
                && self.passcode_key_salt.is_empty()
                && self.passcode_key_encrypted.is_empty()
        );
        let mut pass = [0; 256];
        let mut salt = [0; 32];
        random.fill(&mut pass);
        random.fill(&mut salt);
        self.local_key = Some(create_local_key(&pass, &salt));
        self.encrypt_local_key(&[], random)
    }
    pub fn encrypt_local_key(&mut self, passcode: &[u8], random: &mut impl Random) -> Result<()> {
        self.passcode_key_salt.resize(32, 0);
        random.fill(&mut self.passcode_key_salt);
        let key = create_local_key(passcode, &self.passcode_key_salt);
        let local = self
            .local_key
            .as_ref()
            .ok_or(Error::Invalid("local key missing"))?;
        let mut data = EncryptedDescriptor::new(256);
        data.stream.0.extend(local.data());
        self.passcode_key_encrypted = prepare_encrypted(&mut data, &key, random);
        self.passcode_key = Some(key);
        self.has_passcode = !passcode.is_empty();
        Ok(())
    }
    pub fn write_accounts(
        &self,
        manager: &mut WriteManager,
        random: &mut impl Random,
    ) -> Result<()> {
        assert!(!self.owner.accounts.is_empty());
        let mut file = FileWriteDescriptor::new(&compute_key_name(&self.data_name), &self.root);
        file.write_data(Some(&self.passcode_key_salt));
        file.write_data(Some(&self.passcode_key_encrypted));
        let mut data = EncryptedDescriptor::new(4 + 4 * self.owner.accounts.len());
        data.stream.i32(self.owner.accounts.len() as i32);
        for (index, _) in &self.owner.accounts {
            data.stream.i32(*index);
        }
        data.stream.i32(self.owner.active_for_storage());
        file.write_encrypted(
            &mut data,
            self.local_key
                .as_ref()
                .ok_or(Error::Invalid("local key missing"))?,
            random,
        );
        file.finish(manager, false)
    }
    pub fn start_added(&self, account: &mut Account) {
        account.start_added(self.local_key.as_ref().expect("local key").clone());
    }
    pub fn start_with_single_account(
        &mut self,
        passcode: &[u8],
        mut account: Account,
        random: &mut impl Random,
        manager: &mut WriteManager,
    ) -> Result<()> {
        if let Some(key) = account.local_key.clone() {
            self.local_key = Some(key);
            self.encrypt_local_key(passcode, random)?;
        } else {
            self.generate_local_key(random)?;
            account.start_added(self.local_key.as_ref().unwrap().clone());
        }
        self.owner
            .account_added_in_storage(0, account.authorization)?;
        self.write_accounts(manager, random)
    }
    pub fn start_from_scratch(
        &mut self,
        random: &mut impl Random,
        manager: &mut WriteManager,
    ) -> Result<()> {
        let account = Account::new(&self.root, &compose_data_string(&self.data_name, 0));
        self.start_with_single_account(&[], account, random, manager)
    }
    pub fn check_passcode(&self, passcode: &[u8]) -> bool {
        assert!(!self.passcode_key_salt.is_empty());
        create_local_key(passcode, &self.passcode_key_salt)
            .equals(self.passcode_key.as_ref().expect("passcode key"))
    }
    pub fn set_passcode(
        &mut self,
        passcode: &[u8],
        manager: &mut WriteManager,
        random: &mut impl Random,
    ) -> Result<()> {
        self.encrypt_local_key(passcode, random)?;
        self.write_accounts(manager, random)?;
        self.changes.retain(|tx| tx.send(()).is_ok());
        Ok(())
    }
    pub fn old_version(&self) -> i32 {
        self.old_version
    }
    pub fn clear_old_version(&mut self) {
        self.old_version = 0;
    }
    pub fn has_local_passcode(&self) -> bool {
        self.has_passcode
    }
    pub fn local_passcode_changed(&mut self) -> mpsc::Receiver<()> {
        let (tx, rx) = mpsc::channel();
        self.changes.push(tx);
        rx
    }
}
