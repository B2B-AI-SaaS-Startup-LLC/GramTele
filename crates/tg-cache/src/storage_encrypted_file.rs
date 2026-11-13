use std::{
    fs::{self, File as DataFile, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};
use tg_base::{file_lock::FileLock, openssl_help::sha256_parts, random::random_fill};

use crate::storage_encryption::{CtrState, EncryptionKey, SALT_SIZE};

const HEADER_SIZE: usize = SALT_SIZE + 16 + 32;
const CHECKSUM_OFFSET: usize = HEADER_SIZE - 32;
const HEADER_ENCRYPTED_SIZE: usize = HEADER_SIZE - SALT_SIZE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Read,
    ReadAppend,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Result {
    Failed,
    LockFailed,
    WrongKey,
    Success,
}

#[derive(Default)]
pub struct File {
    data: Option<DataFile>,
    lock: FileLock,
    encryption_offset: i64,
    data_size: i64,
    state: Option<CtrState>,
    random: Option<std::sync::Arc<dyn RandomSource>>,
    // QFileDevice buffers writes up to 16 KiB, flushes on seek/read/close,
    // and retains unwritten bytes after a short flush. `pos` is logical.
    pending: Vec<u8>,
    plain_position: i64,
    mode: Option<Mode>,
}

/// Entropy for salts, value names and padding. Production uses the OS source;
/// an explicit source permits reproducible native format tests without globals.
pub trait RandomSource: Send + Sync {
    fn fill(&self, bytes: &mut [u8]);
}
pub struct SystemRandom;
impl RandomSource for SystemRandom {
    fn fill(&self, bytes: &mut [u8]) {
        random_fill(bytes);
    }
}

impl File {
    pub fn with_random(random: std::sync::Arc<dyn RandomSource>) -> Self {
        let mut result = Self::default();
        result.random = Some(random);
        result
    }

    fn fill_random(&self, bytes: &mut [u8]) {
        if let Some(random) = &self.random {
            random.fill(bytes);
        } else {
            random_fill(bytes);
        }
    }
    pub fn open(&mut self, path: &Path, mode: Mode, key: &EncryptionKey) -> Result {
        self.close();
        let Ok(path) = std::path::absolute(path) else {
            return Result::Failed;
        };
        if mode != Mode::Read {
            if let Some(parent) = path.parent() {
                if fs::create_dir_all(parent).is_err() {
                    return Result::Failed;
                }
            }
        }
        let result = self.attempt_open(&path, mode, key);
        if result != Result::Success {
            self.close();
        }
        result
    }

    fn attempt_open(&mut self, path: &Path, mode: Mode, key: &EncryptionKey) -> Result {
        self.mode = Some(mode);
        if mode == Mode::Read {
            let Ok(file) = DataFile::open(path) else {
                return Result::Failed;
            };
            self.data = Some(file);
            return self.read_header(key);
        }
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create(true)
            .truncate(mode == Mode::Write)
            .read(mode == Mode::ReadAppend);
        if !self.lock.lock(&mut self.data, path, &options) {
            return Result::LockFailed;
        }
        if mode == Mode::Write
            || self
                .data
                .as_ref()
                .and_then(|file| file.metadata().ok())
                .is_some_and(|v| v.len() == 0)
        {
            if self.write_header(key) {
                Result::Success
            } else {
                Result::Failed
            }
        } else {
            self.read_header(key)
        }
    }

    fn write_header(&mut self, key: &EncryptionKey) -> bool {
        assert!(self.state.is_none());
        assert_eq!(self.position(), Some(0));
        if self.write_plain(b"TDEF") == 0 {
            return false;
        }
        let mut header = [0; HEADER_SIZE];
        self.fill_random(&mut header[..SALT_SIZE]);
        self.state = Some(key.prepare_ctr_state(&header[..SALT_SIZE]));
        let checksum = sha256_parts(&[key.data(), &header[..CHECKSUM_OFFSET]]);
        header[CHECKSUM_OFFSET..].copy_from_slice(&checksum);
        if self.write_plain(&header[..SALT_SIZE]) != SALT_SIZE as isize
            || !self.write(&mut header[SALT_SIZE..])
        {
            return false;
        }
        self.data_size = 0;
        true
    }

    fn read_header(&mut self, key: &EncryptionKey) -> Result {
        assert!(self.state.is_none());
        assert_eq!(self.position(), Some(0));
        if !self.seek_plain(FileLock::SKIP_BYTES as i64) {
            return Result::Failed;
        }
        let mut header = [0; HEADER_SIZE];
        if self.read_plain(&mut header) != HEADER_SIZE as isize {
            return Result::Failed;
        }
        self.state = Some(key.prepare_ctr_state(&header[..SALT_SIZE]));
        self.decrypt(&mut header[SALT_SIZE..]);
        let checksum = sha256_parts(&[key.data(), &header[..CHECKSUM_OFFSET]]);
        if header[CHECKSUM_OFFSET..] != checksum {
            return Result::WrongKey;
        }
        if header[SALT_SIZE] != 0 {
            return Result::Failed;
        }
        let Some(size) = self
            .data
            .as_ref()
            .and_then(|file| file.metadata().ok())
            .map(|v| v.len() as i64)
        else {
            return Result::Failed;
        };
        self.data_size = size - HEADER_SIZE as i64 - FileLock::SKIP_BYTES as i64;
        assert!(self.data_size >= 0);
        self.data_size -= self.data_size % CtrState::BLOCK_SIZE as i64;
        Result::Success
    }

    fn position(&mut self) -> Option<i64> {
        self.data.as_ref().map(|_| self.plain_position)
    }

    fn seek_plain(&mut self, offset: i64) -> bool {
        let success = offset >= 0
            && self.flush()
            && self
                .data
                .as_mut()
                .is_some_and(|file| file.seek(SeekFrom::Start(offset as u64)).is_ok());
        if success {
            self.plain_position = offset;
        }
        success
    }

    fn read_plain(&mut self, bytes: &mut [u8]) -> isize {
        if self.mode == Some(Mode::Write) || !self.flush() {
            return -1;
        }
        let count = self.data.as_mut().map_or(-1, |file| {
            let mut count = 0;
            while count < bytes.len() {
                match file.read(&mut bytes[count..]) {
                    Ok(0) => break,
                    Ok(n) => count += n,
                    Err(_) => return if count == 0 { -1 } else { count as isize },
                }
            }
            count as isize
        });
        if count > 0 {
            self.plain_position += count as i64;
        }
        count
    }

    fn write_plain(&mut self, bytes: &[u8]) -> isize {
        if self.data.is_none() || self.mode == Some(Mode::Read) {
            return -1;
        }
        const CHUNK: usize = 16384;
        if self.pending.len() + bytes.len() > CHUNK && !self.flush() {
            return -1;
        }
        let count = if bytes.len() > CHUNK {
            Self::write_engine(self.data.as_mut().unwrap(), bytes)
        } else {
            self.pending.extend_from_slice(bytes);
            bytes.len() as isize
        };
        if count > 0 {
            self.plain_position += count as i64;
        }
        count
    }

    fn decrypt(&mut self, bytes: &mut [u8]) {
        self.state
            .as_ref()
            .expect("File: cipher state missing")
            .decrypt(bytes, self.encryption_offset);
        self.encryption_offset += bytes.len() as i64;
    }

    fn encrypt(&mut self, bytes: &mut [u8]) {
        self.state
            .as_ref()
            .expect("File: cipher state missing")
            .encrypt(bytes, self.encryption_offset);
        self.encryption_offset += bytes.len() as i64;
    }

    fn decrypt_back(&mut self, bytes: &mut [u8]) {
        assert!(self.encryption_offset >= bytes.len() as i64);
        self.encryption_offset -= bytes.len() as i64;
        self.decrypt(bytes);
        self.encryption_offset -= bytes.len() as i64;
    }

    pub fn read(&mut self, bytes: &mut [u8]) -> isize {
        assert!(bytes.len().is_multiple_of(CtrState::BLOCK_SIZE));
        let mut count = self.read_plain(bytes);
        let back = -(count % CtrState::BLOCK_SIZE as isize);
        if back != 0 {
            let position = self.position().unwrap_or(0);
            if !self.seek_plain(position + back as i64) {
                return 0;
            }
            count += back;
        }
        if count != 0 {
            self.decrypt(&mut bytes[..count as usize]);
        }
        count
    }

    pub fn write(&mut self, bytes: &mut [u8]) -> bool {
        assert!(bytes.len().is_multiple_of(CtrState::BLOCK_SIZE));
        if !self.is_open() {
            return false;
        }
        self.encrypt(bytes);
        let count = self.write_plain(bytes);
        if count == bytes.len() as isize {
            self.data_size = self.data_size.max(self.offset());
            true
        } else {
            self.decrypt_back(bytes);
            if count > 0 {
                if let Some(position) = self.position() {
                    self.seek_plain(position - count as i64);
                }
            }
            false
        }
    }

    pub fn read_with_padding(&mut self, bytes: &mut [u8]) -> isize {
        let part = bytes.len() % CtrState::BLOCK_SIZE;
        let good = bytes.len() - part;
        if good != 0 {
            let count = self.read(&mut bytes[..good]);
            if count != good as isize {
                return count;
            }
        }
        if part == 0 {
            return good as isize;
        }
        let mut padded = [0; CtrState::BLOCK_SIZE];
        let count = self.read(&mut padded);
        if count == 0 {
            return good as isize;
        }
        assert_eq!(count, CtrState::BLOCK_SIZE as isize);
        bytes[good..].copy_from_slice(&padded[..part]);
        bytes.len() as isize
    }

    pub fn write_with_padding(&mut self, bytes: &mut [u8]) -> bool {
        let part = bytes.len() % CtrState::BLOCK_SIZE;
        let good = bytes.len() - part;
        if good != 0 && !self.write(&mut bytes[..good]) {
            return false;
        }
        if part == 0 {
            return true;
        }
        let mut padded = [0; CtrState::BLOCK_SIZE];
        padded[..part].copy_from_slice(&bytes[good..]);
        self.fill_random(&mut padded[part..]);
        if self.write(&mut padded) {
            return true;
        }
        if good != 0 {
            self.decrypt_back(&mut bytes[..good]);
            if let Some(position) = self.position() {
                self.seek_plain(position - good as i64);
            }
        }
        false
    }

    pub fn flush(&mut self) -> bool {
        let Some(file) = self.data.as_mut() else {
            return false;
        };
        if !self.pending.is_empty() {
            let count = Self::write_engine(file, &self.pending).max(0) as usize;
            let full = count == self.pending.len();
            self.pending.drain(..count);
            if !full {
                return false;
            }
        }
        file.flush().is_ok()
    }

    fn write_engine(file: &mut DataFile, bytes: &[u8]) -> isize {
        let mut written = 0;
        while written < bytes.len() {
            match file.write(&bytes[written..]) {
                Ok(0) | Err(_) => return if written == 0 { -1 } else { written as isize },
                Ok(n) => written += n,
            }
        }
        written as isize
    }

    pub fn is_open(&self) -> bool {
        self.data.is_some()
    }
    pub fn size(&self) -> i64 {
        self.data_size
    }
    pub fn offset(&self) -> i64 {
        (SALT_SIZE as i64 + self.encryption_offset - HEADER_SIZE as i64).max(0)
    }

    pub fn seek(&mut self, offset: i64) -> bool {
        if offset < 0
            || offset > self.data_size
            || !self.seek_plain(FileLock::SKIP_BYTES as i64 + HEADER_SIZE as i64 + offset)
        {
            return false;
        }
        self.encryption_offset = HEADER_ENCRYPTED_SIZE as i64 + offset;
        true
    }

    pub fn close(&mut self) {
        self.lock.unlock();
        self.flush();
        self.data = None;
        self.data_size = 0;
        self.encryption_offset = 0;
        self.state = None;
        self.pending.clear();
        self.plain_position = 0;
        self.mode = None;
    }

    pub fn move_file(from: &Path, to: &Path) -> bool {
        if !from.exists() {
            return false;
        }
        if to.exists() {
            let mut destination = None;
            let mut lock = FileLock::new();
            let mut options = OpenOptions::new();
            options.write(true).create(true).truncate(true);
            if !lock.lock(&mut destination, to, &options) {
                return false;
            }
            lock.unlock();
            drop(destination);
            if fs::remove_file(to).is_err() {
                return false;
            }
        }
        if Self::rename_no_replace(from, to).is_ok() {
            return true;
        }
        if from.is_dir() {
            return false;
        }
        Self::copy_and_remove(from, to).is_ok()
    }

    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
        use nix::fcntl::{renameat2, RenameFlags, AT_FDCWD};
        renameat2(AT_FDCWD, from, AT_FDCWD, to, RenameFlags::RENAME_NOREPLACE)
            .map_err(io::Error::from)
    }

    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
        if from.is_dir() {
            if to.exists() {
                return Err(io::Error::from(io::ErrorKind::AlreadyExists));
            }
            return fs::rename(from, to);
        }
        if fs::hard_link(from, to).is_ok() {
            if let Err(error) = fs::remove_file(from) {
                let _ = fs::remove_file(to);
                return Err(error);
            }
            return Ok(());
        }
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    fn copy_and_remove(from: &Path, to: &Path) -> io::Result<()> {
        let mut source = DataFile::open(from)?;
        let mut target = OpenOptions::new().write(true).create_new(true).open(to)?;
        let copied = io::copy(&mut source, &mut target)
            .and_then(|_| target.flush())
            .and_then(|_| target.set_permissions(source.metadata()?.permissions()));
        drop(target);
        if let Err(error) = copied.and_then(|_| fs::remove_file(from)) {
            let _ = fs::remove_file(to);
            return Err(error);
        }
        Ok(())
    }
}

impl Drop for File {
    fn drop(&mut self) {
        self.close();
    }
}
