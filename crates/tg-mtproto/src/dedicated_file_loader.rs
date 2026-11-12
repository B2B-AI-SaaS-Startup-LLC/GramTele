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
        std::fs::create_dir_all(parent)?;
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
    failed: bool,
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
            failed: false,
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
        if self.failed || self.requests.len() >= 6 || self.offset >= self.size {
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
                self.failed = true;
                self.output.thread_safe_failed(error);
            }
        }
    }
}
