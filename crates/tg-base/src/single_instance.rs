//! Linux local-socket core of base/single_instance.{h,cpp}.
//! Qt event delivery is replaced by explicit nonblocking process()/poll_reply().
//! Socket name and valid wire messages retain Qt's native little-endian format.
#![cfg(target_os = "linux")]
use crate::crc32hash::crc32;
use crate::file_lock::FileLock;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use tg_rpl::{EventStream, Producer};

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
fn base64(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = chunk.get(1).copied().unwrap_or(0);
        let c = chunk.get(2).copied().unwrap_or(0);
        out.push(ALPHABET[(a >> 2) as usize]);
        out.push(ALPHABET[(((a & 3) << 4) | (b >> 4)) as usize]);
        out.push(if chunk.len() > 1 {
            ALPHABET[(((b & 15) << 2) | (c >> 6)) as usize]
        } else {
            b'='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(c & 63) as usize]
        } else {
            b'='
        });
    }
    out
}
pub fn encode_message(message: &[u8]) -> Vec<u8> {
    let mut result = base64(message);
    result.push(b';');
    result
}
pub fn decode_message(message: &[u8]) -> Option<Vec<u8>> {
    if !message.ends_with(b";") {
        return None;
    }
    let mut result = Vec::new();
    let mut accumulator = 0u32;
    let mut bits = 0;
    for &byte in &message[..message.len() - 1] {
        // Qt's default IgnoreBase64DecodingErrors skips non-alphabet bytes.
        if let Some(value) = ALPHABET.iter().position(|&c| c == byte) {
            accumulator = (accumulator << 6) | value as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                result.push((accumulator >> bits) as u8);
            }
        }
    }
    Some(result)
}
pub fn name_for_path(application: &str, path: &str) -> String {
    let hash = |text: &str| {
        let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        crc32(&bytes)
    };
    let mut bytes = hash(application).to_le_bytes().to_vec();
    bytes.extend_from_slice(&hash(path).to_le_bytes());
    String::from_utf8(base64(&bytes)).unwrap()
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub id: u32,
    pub data: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reply {
    pub pid: u64,
    pub window: u64,
}
struct Client {
    socket: UnixStream,
    input: Vec<u8>,
    output: Vec<u8>,
    written: usize,
}
impl Client {
    fn new(socket: UnixStream) -> io::Result<Self> {
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            input: Vec::new(),
            output: Vec::new(),
            written: 0,
        })
    }
    fn read(&mut self) -> io::Result<bool> {
        let mut buffer = [0; 4096];
        loop {
            match self.socket.read(&mut buffer) {
                Ok(0) => return Ok(false),
                Ok(n) => self.input.extend_from_slice(&buffer[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(true),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
    }
    fn write(&mut self) -> io::Result<()> {
        while self.written < self.output.len() {
            match self.socket.write(&self.output[self.written..]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => self.written += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}
pub struct SingleInstance {
    listener: Option<UnixListener>,
    secondary: RefCell<Option<Client>>,
    clients: RefCell<BTreeMap<u32, Client>>,
    last_id: std::cell::Cell<u32>,
    commands: EventStream<Message>,
    socket_path: PathBuf,
    lock_path: PathBuf,
    lock: FileLock,
    _file: Option<File>,
}
impl SingleInstance {
    pub fn start(application: &str, path: &str) -> io::Result<Self> {
        Self::start_in(application, path, &std::env::temp_dir())
    }
    /// Explicit temporary directory for tests; production Qt uses QDir::tempPath.
    pub fn start_in(application: &str, path: &str, socket_dir: &Path) -> io::Result<Self> {
        let name = name_for_path(application, path);
        let socket_path = socket_dir.join(&name);
        let lock_path = PathBuf::from(format!("{path}_single_instance.tmp"));
        let mut lock = FileLock::new();
        let mut file = None;
        let (listener, secondary) = match UnixStream::connect(&socket_path) {
            Ok(socket) => (None, Some(Client::new(socket)?)),
            Err(_) => {
                let mut options = OpenOptions::new();
                options.write(true).create(true).truncate(true);
                if !lock.lock(&mut file, &lock_path, &options) {
                    return Err(io::Error::other("single-instance lock failed"));
                }
                // CleanName in the original is relative to cwd, not tempPath.
                // Keep that edge case: a stale socket elsewhere can fail listen.
                let _ = std::fs::remove_file(&name);
                let listener = UnixListener::bind(&socket_path)?;
                listener.set_nonblocking(true)?;
                (Some(listener), None)
            }
        };
        Ok(Self {
            listener,
            secondary: RefCell::new(secondary),
            clients: RefCell::new(BTreeMap::new()),
            last_id: std::cell::Cell::new(0),
            commands: EventStream::new(),
            socket_path,
            lock_path,
            lock,
            _file: file,
        })
    }
    pub fn is_primary(&self) -> bool {
        self.listener.is_some()
    }
    pub fn commands(&self) -> Producer<Message> {
        self.commands.events()
    }
    /// Called by the owner's loop after socket readiness. No callbacks under a borrow.
    pub fn process(&self) -> io::Result<usize> {
        let listener = self
            .listener
            .as_ref()
            .ok_or_else(|| io::Error::other("secondary instance"))?;
        loop {
            match listener.accept() {
                Ok((socket, _)) => {
                    let id = self.last_id.get().wrapping_add(1);
                    self.last_id.set(id);
                    self.clients.borrow_mut().insert(id, Client::new(socket)?);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        let mut messages = Vec::new();
        {
            let mut clients = self.clients.borrow_mut();
            let mut disconnected = Vec::new();
            for (&id, client) in clients.iter_mut() {
                let old = client.input.len();
                let live = client.read()?;
                client.write()?;
                if client.input.len() != old {
                    if let Some(data) = decode_message(&client.input) {
                        messages.push(Message { id, data });
                    }
                }
                if !live {
                    disconnected.push(id);
                }
            }
            for id in disconnected {
                clients.remove(&id);
            }
        }
        let count = messages.len();
        for message in messages {
            self.commands.fire(message);
        }
        Ok(count)
    }
    pub fn send(&self, command: &[u8]) -> io::Result<()> {
        let mut secondary = self.secondary.borrow_mut();
        let client = secondary
            .as_mut()
            .ok_or_else(|| io::Error::other("primary instance"))?;
        client.output = encode_message(command);
        client.written = 0;
        client.input.clear();
        client.write()
    }
    /// QWidget activation is left to tg-window; returns the wire PID/window pair.
    pub fn poll_reply(&self) -> io::Result<Option<Reply>> {
        let mut secondary = self.secondary.borrow_mut();
        let client = secondary
            .as_mut()
            .ok_or_else(|| io::Error::other("primary instance"))?;
        client.write()?;
        let live = client.read()?;
        let Some(data) = decode_message(&client.input) else {
            return if live {
                Ok(None)
            } else {
                Err(io::ErrorKind::UnexpectedEof.into())
            };
        };
        let text = String::from_utf8_lossy(&data);
        let parsed = text
            .strip_prefix("PID:")
            .and_then(|s| s.strip_suffix(';'))
            .and_then(|s| s.split_once(";WND:"));
        Ok(Some(
            parsed
                .and_then(|(pid, window)| {
                    Some(Reply {
                        pid: pid.parse().ok()?,
                        window: window.parse().ok()?,
                    })
                })
                .unwrap_or(Reply { pid: 0, window: 0 }),
        ))
    }
    pub fn reply(&self, id: u32, activation: Option<Reply>) -> io::Result<()> {
        let value = activation.unwrap_or(Reply { pid: 0, window: 0 });
        if let Some(client) = self.clients.borrow_mut().get_mut(&id) {
            client.output =
                encode_message(format!("PID:{};WND:{};", value.pid, value.window).as_bytes());
            client.written = 0;
            client.write()?;
        }
        Ok(())
    }
}
impl Drop for SingleInstance {
    fn drop(&mut self) {
        if self.is_primary() {
            self.listener.take();
            let _ = std::fs::remove_file(&self.socket_path);
            self.lock.unlock();
            self._file.take();
            let _ = std::fs::remove_file(&self.lock_path);
        }
    }
}
