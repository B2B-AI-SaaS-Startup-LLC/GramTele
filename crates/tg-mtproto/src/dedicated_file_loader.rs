//! Ordered 1 MiB parts, six requests in flight, resumable output.
use crate::{facade::updater_dc_id, mtp_instance::Instance, sender::Sender};
use std::{
    cell::Cell,
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex},
};
use tg_base::timer::{Timer, TimerHost};
pub const CHUNK_SIZE: usize = 128 * 1024 * 8;
pub const MAX_FILE_SIZE: u64 = 256 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Default, Eq)]
pub struct Progress {
    pub already: u64,
    pub size: u64,
    pub percent: bool,
}
impl PartialEq for Progress {
    fn eq(&self, other: &Self) -> bool {
        (self.already, self.size) == (other.already, other.size)
    }
}
impl Ord for Progress {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.already, self.size).cmp(&(other.already, other.size))
    }
}
impl PartialOrd for Progress {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
#[derive(Clone, Debug)]
pub enum LoaderEvent {
    Progress(Progress),
    Ready(PathBuf),
    Failed(String),
}
pub struct AbstractDedicatedLoader {
    filepath: PathBuf,
    chunk_size: u64,
    output: Option<File>,
    progress: Progress,
    events: VecDeque<LoaderEvent>,
}
impl AbstractDedicatedLoader {
    pub fn new(filepath: PathBuf, chunk_size: u64) -> Self {
        assert!(chunk_size > 0);
        Self {
            filepath,
            chunk_size,
            output: None,
            progress: Progress::default(),
            events: VecDeque::new(),
        }
    }
    pub fn validate_output(&mut self) -> io::Result<()> {
        let Some(parent) = self.filepath.parent() else {
            return Err(io::Error::other("no output directory"));
        };
        if !parent.exists() {
            std::fs::create_dir(parent)?;
        }
        if let Ok(metadata) = std::fs::metadata(&self.filepath) {
            let size = metadata.len();
            if size < self.chunk_size || size > MAX_FILE_SIZE {
                std::fs::remove_file(&self.filepath)?;
            } else {
                let good = size - size % self.chunk_size;
                OpenOptions::new()
                    .write(true)
                    .open(&self.filepath)?
                    .set_len(good)?;
                self.progress.already = good;
            }
        }
        self.output = Some(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.filepath)?,
        );
        Ok(())
    }
    pub fn already_size(&self) -> u64 {
        self.progress.already
    }
    pub fn total_size(&self) -> u64 {
        self.progress.size
    }
    pub fn prefer_percent(&self) -> bool {
        self.progress.percent
    }
    pub fn wipe_folder(&self) -> io::Result<()> {
        let Some(parent) = self.filepath.parent() else {
            return Ok(());
        };
        for entry in std::fs::read_dir(parent)? {
            let entry = entry?;
            if entry.file_type()?.is_file() && entry.path() != self.filepath {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }
    pub fn write_chunk(&mut self, bytes: &[u8], total_size: u64) -> io::Result<()> {
        if total_size > MAX_FILE_SIZE || self.progress.already + bytes.len() as u64 > MAX_FILE_SIZE
        {
            return Err(io::Error::other("Download exceeds the size limit"));
        }
        if !bytes.is_empty() {
            self.output
                .as_mut()
                .ok_or_else(|| io::Error::other("output is not open"))?
                .write_all(bytes)?;
        }
        if self.progress.size == 0 {
            self.progress.size = total_size;
        }
        self.progress.already += bytes.len() as u64;
        if self.progress.size > 0 && self.progress.already >= self.progress.size {
            self.output.take();
            self.events
                .push_back(LoaderEvent::Ready(self.filepath.clone()));
        } else {
            self.events.push_back(LoaderEvent::Progress(self.progress));
        }
        Ok(())
    }
    pub fn thread_safe_failed(&mut self, error: impl ToString) {
        self.events
            .push_back(LoaderEvent::Failed(error.to_string()));
    }
    pub fn take_events(&mut self) -> VecDeque<LoaderEvent> {
        std::mem::take(&mut self.events)
    }
    pub fn filepath(&self) -> &Path {
        &self.filepath
    }
}
pub struct FileDescription {
    pub name: String,
    pub size: u64,
    pub dc_id: i32,
    pub location: tg_scheme::InputFileLocation,
}
type PartResult = Result<(u64, tg_scheme::UploadFile), String>;
pub struct DedicatedLoader {
    pub output: AbstractDedicatedLoader,
    sender: Sender,
    size: u64,
    dc: i32,
    location: tg_scheme::InputFileLocation,
    offset: u64,
    requests: VecDeque<(u64, Vec<u8>)>,
    parts: Arc<Mutex<VecDeque<PartResult>>>,
    timer: Timer,
    due: Rc<Cell<bool>>,
}
impl DedicatedLoader {
    pub fn new(
        instance: Instance,
        folder: &Path,
        file: FileDescription,
        host: &Rc<TimerHost>,
    ) -> Self {
        assert!(file.size > 0);
        let due = Rc::new(Cell::new(false));
        let wake = due.clone();
        Self {
            output: AbstractDedicatedLoader::new(
                folder.join(tg_base::base_file_utilities::file_name_from_user_string(
                    &file.name,
                )),
                CHUNK_SIZE as u64,
            ),
            sender: Sender::new(instance),
            size: file.size,
            dc: file.dc_id,
            location: file.location,
            offset: 0,
            requests: VecDeque::new(),
            parts: Arc::new(Mutex::new(VecDeque::new())),
            timer: Timer::in_host(host, Some(Rc::new(move || wake.set(true)))),
            due,
        }
    }
    pub fn start_loading(&mut self) -> io::Result<()> {
        self.output.validate_output()?;
        self.offset = self.output.already_size();
        self.output.write_chunk(&[], self.size)?;
        self.send_request();
        Ok(())
    }
    pub fn send_request(&mut self) {
        if self.requests.len() >= 6 || self.offset >= self.size {
            return;
        }
        let offset = self.offset;
        self.requests.push_back((offset, vec![]));
        let done = self.parts.clone();
        let fail = self.parts.clone();
        self.sender
            .request(tg_scheme::UploadGetFile::new(
                tg_tl::Flags::default(),
                self.location.clone(),
                offset,
                CHUNK_SIZE as i32,
            ))
            .to_dc(updater_dc_id(self.dc))
            .done(move |result| done.lock().unwrap().push_back(Ok((offset, result))))
            .fail(move |error| fail.lock().unwrap().push_back(Err(error.kind.clone())))
            .send();
        self.offset += CHUNK_SIZE as u64;
        if self.requests.len() < 6 {
            self.timer.call_once(20);
        }
    }
    pub fn got_part(&mut self, offset: u64, result: tg_scheme::UploadFile) -> io::Result<()> {
        let tg_scheme::UploadFile::UploadFile(data) = result else {
            return Err(io::Error::other("MTP does not support cdn right now."));
        };
        if data.bytes.is_empty() {
            return Err(io::Error::other("MTP empty part received."));
        }
        let entry = self
            .requests
            .iter_mut()
            .find(|(at, _)| *at == offset)
            .expect("unexpected offset");
        entry.1 = data.bytes.clone();
        while self
            .requests
            .front()
            .is_some_and(|(_, bytes)| !bytes.is_empty())
        {
            let (_, bytes) = self.requests.pop_front().unwrap();
            self.output.write_chunk(&bytes, self.size)?;
        }
        self.send_request();
        Ok(())
    }
    pub fn process(&mut self) {
        if self.due.replace(false) {
            self.send_request();
        }
        let parts = std::mem::take(&mut *self.parts.lock().unwrap());
        for part in parts {
            let result = match part {
                Ok((offset, result)) => self.got_part(offset, result),
                Err(error) => Err(io::Error::other(error)),
            };
            if let Err(error) = result {
                self.output.thread_safe_failed(error);
            }
        }
    }
}

pub fn get_messages_element(
    list: &tg_scheme::MessagesMessages,
    id: i32,
) -> Option<tg_scheme::Message> {
    use tg_scheme::MessagesMessages::*;
    let messages = match list {
        MessagesMessages(d) => &d.messages,
        MessagesMessagesSlice(d) => &d.messages,
        MessagesChannelMessages(d) => &d.messages,
        MessagesMessagesNotModified(_) => return None,
    };
    messages
        .iter()
        .find(|message| {
            let actual = match message {
                tg_scheme::Message::MessageEmpty(d) => d.id,
                tg_scheme::Message::Message(d) => d.id,
                tg_scheme::Message::MessageService(d) => d.id,
            };
            id == 0 || id == actual
        })
        .cloned()
}
pub fn parse_file(list: &tg_scheme::MessagesMessages, post_id: i32) -> Option<FileDescription> {
    let tg_scheme::Message::Message(message) = get_messages_element(list, post_id)? else {
        return None;
    };
    let tg_scheme::MessageMedia::MessageMediaDocument(media) = message.vmedia()? else {
        return None;
    };
    let tg_scheme::Document::Document(document) = media.vdocument()? else {
        return None;
    };
    let name = document.attributes.iter().find_map(|attribute| {
        if let tg_scheme::DocumentAttribute::DocumentAttributeFilename(d) = attribute {
            Some(String::from_utf8_lossy(&d.file_name).into_owned())
        } else {
            None
        }
    })?;
    if name.is_empty() || document.size == 0 || document.size > MAX_FILE_SIZE {
        return None;
    }
    Some(FileDescription {
        name,
        size: document.size,
        dc_id: document.dc_id,
        location: tg_scheme::mtp_input_document_file_location(
            document.id,
            document.access_hash,
            document.file_reference.clone(),
            vec![],
        ),
    })
}
pub fn extract_channel(
    result: &tg_scheme::ContactsResolvedPeer,
) -> Option<tg_scheme::InputChannel> {
    let data = result.c_contacts_resolved_peer();
    let tg_scheme::Peer::PeerChannel(peer) = &data.peer else {
        return None;
    };
    data.chats.iter().find_map(|chat| {
        if let tg_scheme::Chat::Channel(channel) = chat {
            if channel.id == peer.channel_id {
                return Some(tg_scheme::mtp_input_channel(
                    channel.id,
                    channel.vaccess_hash().unwrap_or_default(),
                ));
            }
        }
        None
    })
}
type WeakFailure = Arc<dyn Fn(&crate::mtproto_response::Error) + Send + Sync>;
pub struct WeakInstance {
    pending: Arc<Mutex<std::collections::BTreeMap<i32, WeakFailure>>>,
    sender: Sender,
    owner: std::sync::Weak<()>,
    alive: Arc<std::sync::atomic::AtomicBool>,
    object_alive: Arc<std::sync::atomic::AtomicBool>,
}
impl WeakInstance {
    pub fn new(instance: Instance, owner: std::sync::Weak<()>) -> Self {
        Self {
            pending: Arc::new(Mutex::new(std::collections::BTreeMap::new())),
            sender: Sender::new_weak(instance),
            owner,
            alive: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            object_alive: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }
    pub fn valid(&self) -> bool {
        self.owner.strong_count() > 0
            && self.alive.load(std::sync::atomic::Ordering::Acquire)
            && self.sender.upgrade_instance().is_some()
    }
    pub fn instance(&self) -> Option<Instance> {
        if self.valid() {
            self.sender.upgrade_instance()
        } else {
            None
        }
    }
    pub fn session(&self) -> std::sync::Weak<()> {
        self.owner.clone()
    }
    pub fn die(&self) {
        self.alive
            .store(false, std::sync::atomic::Ordering::Release);
        let pending = std::mem::take(&mut *self.pending.lock().unwrap());
        for (id, fail) in pending {
            self.sender.cancel(id);
            fail(&crate::mtproto_response::Error::local(
                "UNAVAILABLE",
                "MTP instance is not available.",
            ));
        }
    }
    pub fn process(&self) {
        if !self.valid() {
            self.die();
        }
    }
    pub fn send<R: tg_tl::Function>(
        &self,
        request: R,
        done: impl FnOnce(R::Response) + Send + 'static,
        fail: impl Fn(&crate::mtproto_response::Error) + Send + Sync + 'static,
        dc: i32,
    ) where
        R::Response: Send + 'static,
    {
        let fail: WeakFailure = Arc::new(fail);
        if !self.valid() {
            let alive = Arc::downgrade(&self.object_alive);
            tg_runtime::on_main(move || {
                if alive
                    .upgrade()
                    .is_some_and(|alive| alive.load(std::sync::atomic::Ordering::Acquire))
                {
                    fail(&crate::mtproto_response::Error::local(
                        "UNAVAILABLE",
                        "MTP instance is not available.",
                    ));
                }
            });
            return;
        }
        let owner = self.owner.clone();
        let alive = self.alive.clone();
        let failure_owner = owner.clone();
        let failure_alive = alive.clone();
        let done_pending = self.pending.clone();
        let fail_pending = self.pending.clone();
        let report = fail.clone();
        let id = self
            .sender
            .request(request)
            .to_dc(dc)
            .done_with_id(move |value, id| {
                done_pending.lock().unwrap().remove(&id);
                if owner.strong_count() > 0 && alive.load(std::sync::atomic::Ordering::Acquire) {
                    done(value);
                }
            })
            .fail_with_id(move |error, id| {
                fail_pending.lock().unwrap().remove(&id);
                if failure_owner.strong_count() > 0
                    && failure_alive.load(std::sync::atomic::Ordering::Acquire)
                {
                    fail(error);
                }
            })
            .send();
        self.pending.lock().unwrap().insert(id, report);
    }
}

pub struct Location {
    pub username: String,
    pub channel_id: u64,
    pub access_hash: u64,
    pub post_id: i32,
}
pub fn resolve_channel(
    mtp: Arc<WeakInstance>,
    username: String,
    done: impl FnOnce(tg_scheme::InputChannel) + Send + 'static,
    fail: impl Fn() + Send + Sync + 'static,
) {
    type Cache = std::collections::BTreeMap<String, (std::sync::Weak<()>, tg_scheme::InputChannel)>;
    static CACHE: std::sync::OnceLock<Mutex<Cache>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(Cache::new()));
    if !mtp.valid() {
        fail();
        return;
    }
    let hit = {
        let mut cache = cache.lock().unwrap();
        match cache.get(&username) {
            Some((owner, value)) if owner.ptr_eq(&mtp.owner) => Some(value.clone()),
            _ => {
                cache.remove(&username);
                None
            }
        }
    };
    if let Some(value) = hit {
        done(value);
        return;
    }
    let fail = Arc::new(fail);
    let failed = fail.clone();
    let owner = mtp.owner.clone();
    let cache_name = username.clone();
    mtp.send(
        tg_scheme::ContactsResolveUsername::new(
            tg_tl::Flags::default(),
            username.into_bytes(),
            None,
        ),
        move |result| {
            if let Some(channel) = extract_channel(&result) {
                cache
                    .lock()
                    .unwrap()
                    .insert(cache_name, (owner, channel.clone()));
                done(channel);
            } else {
                fail();
            }
        },
        move |_| failed(),
        0,
    );
}
pub fn start_dedicated_loader(
    mtp: Arc<WeakInstance>,
    location: Location,
    ready: impl FnOnce(Option<FileDescription>) + Send + 'static,
) {
    let ready = Arc::new(Mutex::new(Some(ready)));
    let output = ready.clone();
    let send_mtp = Arc::downgrade(&mtp);
    let request = move |channel| {
        let Some(send_mtp) = send_mtp.upgrade() else {
            return;
        };
        let output = output.clone();
        let fail = output.clone();
        send_mtp.send(
            tg_scheme::ChannelsGetMessages::new(
                channel,
                vec![tg_scheme::mtp_input_message_id(location.post_id)],
            ),
            move |result| {
                if let Some(done) = output.lock().unwrap().take() {
                    done(parse_file(&result, location.post_id));
                }
            },
            move |_| {
                if let Some(done) = fail.lock().unwrap().take() {
                    done(None);
                }
            },
            0,
        );
    };
    if location.channel_id != 0 {
        request(tg_scheme::mtp_input_channel(
            location.channel_id,
            location.access_hash,
        ));
    } else {
        resolve_channel(mtp, location.username, request, move || {
            if let Some(done) = ready.lock().unwrap().take() {
                done(None);
            }
        });
    }
}

impl Drop for WeakInstance {
    fn drop(&mut self) {
        self.object_alive
            .store(false, std::sync::atomic::Ordering::Release);
        self.alive
            .store(false, std::sync::atomic::Ordering::Release);
        self.sender.cancel_all();
    }
}
