//! Shifted DC identifiers and nested pause scopes.
use crate::core_types::*;
pub const fn config_dc_id(id: i32) -> i32 {
    shift_dc_id(id, CONFIG_DC_SHIFT)
}
pub const fn logout_dc_id(id: i32) -> i32 {
    shift_dc_id(id, LOGOUT_DC_SHIFT)
}
pub const fn updater_dc_id(id: i32) -> i32 {
    shift_dc_id(id, UPDATER_DC_SHIFT)
}
pub const fn group_call_stream_dc_id(id: i32) -> i32 {
    shift_dc_id(id, GROUP_CALL_STREAM_DC_SHIFT)
}
pub fn download_dc_id(id: i32, index: i32) -> i32 {
    assert!(index < MAX_MEDIA_DC_COUNT);
    shift_dc_id(id, BASE_DOWNLOAD_DC_SHIFT + index)
}
pub fn upload_dc_id(index: i32) -> i32 {
    assert!((0..MAX_MEDIA_DC_COUNT).contains(&index));
    shift_dc_id(0, BASE_UPLOAD_DC_SHIFT + index)
}
pub const fn is_download_dc_id(id: i32) -> bool {
    id >= BASE_DOWNLOAD_DC_SHIFT * DC_SHIFT
        && id < (BASE_DOWNLOAD_DC_SHIFT + MAX_MEDIA_DC_COUNT) * DC_SHIFT
}
pub const fn is_upload_dc_id(id: i32) -> bool {
    id >= BASE_UPLOAD_DC_SHIFT * DC_SHIFT
        && id < (BASE_UPLOAD_DC_SHIFT + MAX_MEDIA_DC_COUNT) * DC_SHIFT
}
pub const fn is_media_cluster_dc_id(id: i32) -> bool {
    is_download_dc_id(id)
        || matches!(
            get_dc_id_shift(id),
            GROUP_CALL_STREAM_DC_SHIFT | EXPORT_MEDIA_DC_SHIFT | UPDATER_DC_SHIFT
        )
}
pub const fn is_cdn_dc(flags: u32) -> bool {
    flags & 8 != 0
}
pub const fn is_temporary_dc_id(id: i32) -> bool {
    bare_dc_id(id) >= 1000
}
pub const fn get_real_id_from_temporary_dc_id(id: i32) -> i32 {
    let id = bare_dc_id(id);
    if id >= 1000 {
        id - 1000
    } else {
        0
    }
}
pub const fn get_temporary_id_from_real_dc_id(id: i32) -> i32 {
    let id = bare_dc_id(id);
    if id < 1000 {
        id + 1000
    } else {
        0
    }
}
pub const fn destroy_key_next_dc_id(id: i32) -> i32 {
    let shift = get_dc_id_shift(id);
    shift_dc_id(
        bare_dc_id(id),
        if shift == 0 {
            DESTROY_KEY_START_DC_SHIFT
        } else {
            shift + 1
        },
    )
}
#[derive(Default)]
pub struct Pause {
    level: i32,
    subscribers: Vec<tokio::sync::mpsc::UnboundedSender<()>>,
}
impl Pause {
    pub fn paused(&self) -> bool {
        self.level > 0
    }
    pub fn pause(&mut self) {
        self.level += 1;
    }
    pub fn unpause(&mut self) {
        self.level -= 1;
        if self.level == 0 {
            self.subscribers.retain(|tx| tx.send(()).is_ok());
        }
    }
    pub fn unpaused(&mut self) -> tokio::sync::mpsc::UnboundedReceiver<()> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.subscribers.push(tx);
        rx
    }
}
fn global_pause() -> &'static std::sync::Mutex<Pause> {
    static PAUSE: std::sync::OnceLock<std::sync::Mutex<Pause>> = std::sync::OnceLock::new();
    PAUSE.get_or_init(|| std::sync::Mutex::new(Pause::default()))
}
pub fn paused() -> bool {
    global_pause().lock().unwrap().paused()
}
pub fn pause() {
    global_pause().lock().unwrap().pause();
}
pub fn unpause() {
    global_pause().lock().unwrap().unpause();
}
pub fn unpaused() -> tokio::sync::mpsc::UnboundedReceiver<()> {
    global_pause().lock().unwrap().unpaused()
}
