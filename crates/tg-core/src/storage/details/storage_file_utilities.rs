//! TDF$ file envelope and local AES-IGE encryption from storage_file_utilities.
use md5::{Digest, Md5};
use std::{
    collections::VecDeque,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};
use tg_base::{
    openssl_help as crypto,
    qt_stream::{Reader, Writer},
};
use tg_mtproto::{
    core_types::Random,
    mtproto_auth_key::{aes_ige_decrypt_raw, aes_ige_encrypt_raw, AuthKey},
};
pub const APP_VERSION: i32 = 7_002_009;
pub const SALT_SIZE: usize = 32;
pub type FileKey = u64;
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Invalid(&'static str),
    IncorrectPasscode,
    IncorrectPasscodeLegacy,
    Missing,
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
pub type Result<T> = std::result::Result<T, Error>;
pub fn to_file_part(mut val: FileKey) -> String {
    (0..16)
        .map(|_| {
            let c = b"0123456789ABCDEF"[(val & 15) as usize] as char;
            val >>= 4;
            c
        })
        .collect()
}
pub fn key_already_used(name: &mut String) -> bool {
    name.push('0');
    for c in ['0', '1', 's'] {
        name.pop();
        name.push(c);
        if Path::new(name).exists() {
            return true;
        }
    }
    false
}
pub fn generate_key(base: &Path, random: &mut impl Random) -> FileKey {
    loop {
        let mut data = [0; 8];
        random.fill(&mut data);
        let k = u64::from_le_bytes(data);
        let mut name = base.join(to_file_part(k)).to_string_lossy().into_owned();
        if k != 0 && !key_already_used(&mut name) {
            return k;
        }
    }
}
pub fn clear_key(key: FileKey, base: &Path) {
    for suffix in ['0', '1', 's'] {
        let _ = fs::remove_file(base.join(format!("{}{suffix}", to_file_part(key))));
    }
}
pub fn check_stream_status<T>(value: Option<T>) -> Result<T> {
    value.ok_or(Error::Invalid("QDataStream read past end"))
}
pub fn create_local_key(passcode: &[u8], salt: &[u8]) -> Arc<AuthKey> {
    let hash = crypto::sha512_parts(&[salt, passcode, salt]);
    let mut out = [0; 256];
    crypto::pbkdf2_sha512_into(
        &hash,
        salt,
        if passcode.is_empty() { 1 } else { 100_000 },
        &mut out,
    );
    Arc::new(AuthKey::local(out))
}
pub fn create_legacy_local_key(passcode: &[u8], salt: &[u8]) -> Arc<AuthKey> {
    let mut out = [0; 256];
    crypto::pbkdf2_sha1_into(
        passcode,
        salt,
        if passcode.is_empty() { 4 } else { 4000 },
        &mut out,
    );
    Arc::new(AuthKey::local(out))
}
#[derive(Debug)]
pub struct FileReadDescriptor {
    pub version: i32,
    pub data: Vec<u8>,
    pub position: usize,
}
impl FileReadDescriptor {
    pub fn reader(&self) -> Reader<'_> {
        let mut r = Reader::new(&self.data);
        r.position = self.position;
        r
    }
}
pub struct EncryptedDescriptor {
    pub stream: Writer,
}
impl EncryptedDescriptor {
    pub fn new(size: usize) -> Self {
        let mut v = Vec::with_capacity((size + 19) & !15);
        v.resize(4, 0);
        Self { stream: Writer(v) }
    }
    pub fn finish(&mut self) { /* Writer owns bytes; there is no borrowed QBuffer to detach. */
    }
}
pub fn prepare_encrypted(
    data: &mut EncryptedDescriptor,
    key: &AuthKey,
    random: &mut impl Random,
) -> Vec<u8> {
    data.finish();
    let size = data.stream.0.len();
    let full = (size + 15) & !15;
    data.stream.0.resize(full, 0);
    random.fill(&mut data.stream.0[size..]);
    data.stream.0[..4].copy_from_slice(&(size as u32).to_le_bytes());
    let hash = crypto::sha1(&data.stream.0);
    let msg: [u8; 16] = hash[..16].try_into().unwrap();
    let (aes, iv) = key.prepare_aes_oldmtp(&msg, false);
    let mut body = data.stream.0.clone();
    aes_ige_encrypt_raw(&mut body, &aes, &iv).unwrap();
    let mut out = msg.to_vec();
    out.extend(body);
    out
}
pub fn decrypt_local(encrypted: &[u8], key: &AuthKey) -> Result<EncryptedDescriptor> {
    if encrypted.len() <= 16 || !encrypted.len().is_multiple_of(16) {
        return Err(Error::Invalid("bad encrypted part size"));
    }
    let msg: [u8; 16] = encrypted[..16].try_into().unwrap();
    let (aes, iv) = key.prepare_aes_oldmtp(&msg, false);
    let mut out = encrypted[16..].to_vec();
    aes_ige_decrypt_raw(&mut out, &aes, &iv).unwrap();
    if crypto::sha1(&out)[..16] != msg {
        return Err(Error::IncorrectPasscode);
    }
    let size = u32::from_le_bytes(out[..4].try_into().unwrap()) as usize;
    if size > out.len() || size <= out.len() - 16 || size < 4 {
        return Err(Error::Invalid("bad decrypted part size"));
    }
    out.truncate(size);
    Ok(EncryptedDescriptor {
        stream: Writer(out),
    })
}
pub fn encode_file(data: &[u8], version: i32) -> Vec<u8> {
    let mut hash = Md5::new();
    hash.update(data);
    hash.update((data.len() as i32).to_le_bytes());
    hash.update(version.to_le_bytes());
    hash.update(b"TDF$");
    let mut out = b"TDF$".to_vec();
    out.extend(version.to_le_bytes());
    out.extend(data);
    out.extend(hash.finalize());
    out
}
pub fn decode_file(bytes: &[u8]) -> Result<FileReadDescriptor> {
    if bytes.len() < 24 || &bytes[..4] != b"TDF$" {
        return Err(Error::Invalid("bad file magic or size"));
    }
    let version = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if version > APP_VERSION {
        return Err(Error::Invalid("version too big"));
    }
    let data = &bytes[8..bytes.len() - 16];
    if encode_file(data, version) != bytes {
        return Err(Error::Invalid("signature did not match"));
    }
    Ok(FileReadDescriptor {
        version,
        data: data.to_vec(),
        position: 0,
    })
}
pub fn read_file(name: &str, base: &Path) -> Result<FileReadDescriptor> {
    let path = |suffix| base.join(format!("{name}{suffix}"));
    let mut paths = vec![path('s')];
    if !paths[0].exists() {
        paths = vec![path('0'), path('1')];
        if !paths[0].exists() {
            paths.remove(0);
        } else if paths[1].exists() {
            if paths[0].metadata()?.modified()? < paths[1].metadata()?.modified()? {
                paths.swap(0, 1);
            }
        } else {
            paths.pop();
        }
    }
    for (i, p) in paths.iter().enumerate() {
        if let Ok(bytes) = fs::read(p) {
            if let Ok(result) = decode_file(&bytes) {
                if paths.len() == 2 {
                    let _ = fs::remove_file(&paths[1 - i]);
                }
                return Ok(result);
            }
        }
    }
    Err(Error::Missing)
}
pub fn read_encrypted_file(name: &str, base: &Path, key: &AuthKey) -> Result<FileReadDescriptor> {
    let file = read_file(name, base)?;
    let mut r = file.reader();
    let encrypted = check_stream_status(r.bytes())?;
    let result = decrypt_local(&encrypted, key)?;
    Ok(FileReadDescriptor {
        version: file.version,
        data: result.stream.0,
        position: 4,
    })
}
#[derive(Clone)]
pub struct WriteEntry {
    pub base: PathBuf,
    pub data: Vec<u8>,
}
#[derive(Default)]
pub struct WriteManager {
    weak: Option<tg_runtime::WeakOnThread<WriteManager>>,
    scheduled: VecDeque<WriteEntry>,
    finished: bool,
}
impl WriteManager {
    pub fn write(&mut self, entry: WriteEntry) {
        assert!(!self.finished);
        if let Some(old) = self.scheduled.iter_mut().find(|e| e.base == entry.base) {
            *old = entry;
        } else {
            self.scheduled.push_back(entry);
        }
        self.schedule_write();
    }
    pub fn schedule_write(&self) {
        if let Some(weak) = &self.weak {
            weak.with(Self::write_scheduled);
        }
    }
    pub fn write_scheduled(&mut self) {
        if let Err(error) = self.write_one_scheduled_now() {
            eprintln!("tdata write: {error:?}");
        }
        if !self.scheduled.is_empty() {
            self.schedule_write();
        }
    }
    pub fn write_sync(&mut self, entry: WriteEntry) -> Result<()> {
        assert!(!self.finished);
        self.scheduled.retain(|e| e.base != entry.base);
        self.write_now(entry)
    }
    pub fn write_one_scheduled_now(&mut self) -> Result<bool> {
        if let Some(e) = self.scheduled.pop_front() {
            self.write_now(e)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn write_sync_all(&mut self) -> Result<()> {
        let mut error = None;
        while !self.scheduled.is_empty() {
            if let Err(e) = self.write_one_scheduled_now() {
                if error.is_none() {
                    error = Some(e);
                }
            }
        }
        error.map_or(Ok(()), Err)
    }
    pub fn sync(&mut self) -> Result<()> {
        self.write_sync_all()
    }
    pub fn stop(&mut self) -> Result<()> {
        let result = self.sync();
        self.finished = true;
        result
    }
    pub fn write_now(&self, entry: WriteEntry) -> Result<()> {
        use std::io::Write;
        if let Some(parent) = entry.base.parent() {
            fs::create_dir_all(parent)?;
        }
        let path = |c| PathBuf::from(format!("{}{c}", entry.base.display()));
        // QSaveFile: temporary file in the destination directory, then atomic rename.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let temp = PathBuf::from(format!(
            "{}.tmp-{}-{}",
            entry.base.display(),
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let safe = (|| -> io::Result<()> {
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp)?;
            file.write_all(&entry.data)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp, path('s'))?;
            Ok(())
        })();
        if safe.is_ok() {
            let _ = fs::remove_file(path('0'));
            let _ = fs::remove_file(path('1'));
            return Ok(());
        }
        let _ = fs::remove_file(&temp);
        let plain = path('0');
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&plain)?;
        file.write_all(&entry.data)?;
        file.sync_all()?;
        drop(file);
        let _ = fs::remove_file(path('1'));
        if let Err(error) = fs::rename(&plain, path('s')) {
            let _ = fs::remove_file(path('s'));
            return Err(error.into());
        }
        Ok(())
    }
}
pub struct FileWriteDescriptor {
    pub stream: Writer,
    base: PathBuf,
    finished: bool,
}
impl FileWriteDescriptor {
    pub fn new(name: &str, base: &Path) -> Self {
        Self {
            stream: Writer::default(),
            base: base.join(name),
            finished: false,
        }
    }
    pub fn write_data(&mut self, data: Option<&[u8]>) {
        self.stream.nullable_bytes(data);
    }
    pub fn write_encrypted(
        &mut self,
        data: &mut EncryptedDescriptor,
        key: &AuthKey,
        random: &mut impl Random,
    ) {
        self.write_data(Some(&prepare_encrypted(data, key, random)));
    }
    pub fn finish(mut self, manager: &mut WriteManager, sync: bool) -> Result<()> {
        self.finished = true;
        let entry = WriteEntry {
            base: self.base.clone(),
            data: encode_file(&self.stream.0, APP_VERSION),
        };
        if sync {
            manager.write_sync(entry)
        } else {
            manager.write(entry);
            Ok(())
        }
    }
}

#[derive(Default)]
pub struct AsyncWriteManager {
    manager: Option<tg_runtime::ObjectOnThread<WriteManager>>,
    finished: bool,
}
impl AsyncWriteManager {
    fn manager(&mut self) -> &tg_runtime::ObjectOnThread<WriteManager> {
        assert!(!self.finished);
        self.manager.get_or_insert_with(|| {
            tg_runtime::ObjectOnThread::new_with_weak(|weak| WriteManager {
                weak: Some(weak),
                ..Default::default()
            })
        })
    }
    pub fn write(&mut self, entry: WriteEntry) {
        self.manager().with(move |m| m.write(entry));
    }
    pub fn write_sync(&mut self, entry: WriteEntry) -> Result<()> {
        self.manager()
            .with_sync(move |m| m.write_sync(entry))
            .ok_or(Error::Invalid("write manager stopped"))?
    }
    pub fn sync(&mut self) -> Result<()> {
        match &self.manager {
            Some(m) => m
                .with_sync(WriteManager::write_sync_all)
                .ok_or(Error::Invalid("write manager stopped"))?,
            None => Ok(()),
        }
    }
    pub fn stop(&mut self) -> Result<()> {
        let result = self.sync();
        self.manager = None;
        self.finished = true;
        result
    }
}
fn global_manager() -> &'static std::sync::Mutex<AsyncWriteManager> {
    static MANAGER: std::sync::OnceLock<std::sync::Mutex<AsyncWriteManager>> =
        std::sync::OnceLock::new();
    MANAGER.get_or_init(Default::default)
}
pub fn sync() -> Result<()> {
    global_manager().lock().unwrap().sync()
}
pub fn finish() -> Result<()> {
    global_manager().lock().unwrap().stop()
}
impl Drop for FileWriteDescriptor {
    fn drop(&mut self) {
        if !self.finished {
            let entry = WriteEntry {
                base: self.base.clone(),
                data: encode_file(&self.stream.0, APP_VERSION),
            };
            global_manager().lock().unwrap().write(entry);
            self.finished = true;
        }
    }
}
