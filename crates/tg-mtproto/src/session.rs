//! Main-queue boundary of `session.h/.cpp`; no Instance implementation lives here.
use crate::{
    details::{
        mtproto_dcenter::{CreatingKeyType, Dcenter, TemporaryKeyType},
        mtproto_serialized_request::{RequestHandle, RequestStore, SerializedRequest},
    },
    mtproto_auth_key::AuthKey,
    mtproto_dc_options::DcOptions,
    mtproto_proxy_data::{ProxyData, Type},
    mtproto_response::Response,
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::mpsc;
#[derive(Clone, Debug)]
pub struct SessionOptions {
    pub system_lang_code: String,
    pub cloud_lang_code: String,
    pub lang_pack_name: String,
    pub proxy: ProxyData,
    pub use_ipv4: bool,
    pub use_ipv6: bool,
    pub use_http: bool,
    pub use_tcp: bool,
}
impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            system_lang_code: String::new(),
            cloud_lang_code: String::new(),
            lang_pack_name: String::new(),
            proxy: ProxyData::default(),
            use_ipv4: true,
            use_ipv6: true,
            use_http: true,
            use_tcp: true,
        }
    }
}
#[derive(Clone, Default)]
pub struct ProxySettings {
    pub selected: ProxyData,
    pub enabled: bool,
    pub try_ipv6: bool,
}
#[derive(Clone, Debug)]
pub enum EnvironmentChange {
    TemporaryKey(i32),
    DcOptions(i32),
    CdnConfig,
}
/// Each variant is an actual Instance call in session.cpp/session_private.cpp.
#[derive(Clone)]
pub enum InstanceEvent {
    SyncHttpUnixtime,
    RequestConfig,
    RequestConfigIfOld,
    RequestCdnConfig,
    RestartedByTimeout(i32),
    BadConfigurationError,
    KeyWasPossiblyDestroyed(i32),
    KeyDestroyedOnServer(i32, u64),
    DcPersistentKeyChanged(i32, Arc<AuthKey>),
    DcTemporaryKeyChanged(i32),
    StateChange(i32, i32),
    SessionReset(i32),
    Callback(Response),
    Update(Response),
}
/// Read methods mirror actual Instance queries; proxy_settings comes from
/// Core::App().settings().proxy() in Session::refreshOptions.
pub trait SessionEnvironment: Send + Sync + 'static {
    fn dc_options(&self) -> DcOptions;
    fn is_test_mode(&self) -> bool;
    fn is_keys_destroyer(&self) -> bool;
    fn device_model(&self) -> String;
    fn system_version(&self) -> String;
    fn system_lang_code(&self) -> String;
    fn cloud_lang_code(&self) -> String;
    fn lang_pack_name(&self) -> String;
    fn proxy_settings(&self) -> ProxySettings;
    fn has_callback(&self, request_id: i32) -> bool;
    fn subscribe(&self) -> mpsc::UnboundedReceiver<EnvironmentChange>;
    /// Invoked by this crate only through the tg-runtime main queue.
    fn emit(&self, event: InstanceEvent);
}
pub(crate) fn emit_on_main(environment: Arc<dyn SessionEnvironment>, event: InstanceEvent) {
    tg_runtime::on_main(move || environment.emit(event));
}
#[derive(Clone)]
pub enum MainCommand {
    Instance(InstanceEvent),
    TryToReceive,
    NeedToResumeAndSend,
    ConnectionStateChange(i32),
    ResetDone,
    SendAnything(i64),
}
#[derive(Default)]
pub struct RequestQueues {
    pub to_send: BTreeMap<i32, RequestHandle>,
    pub have_sent: BTreeMap<u64, RequestHandle>,
    pub received: VecDeque<Response>,
}
#[derive(Default)]
pub(crate) struct SessionSnapshot {
    pub identity: (u64, u64),
    pub endpoint: Option<crate::mtproto_dc_options::Endpoint>,
    pub init_prefix: Vec<i32>,
    pub state: i32,
    pub retry_at: Option<i64>,
    pub transport: String,
    pub private_alive: bool,
}
pub struct SessionData {
    pub(crate) snapshot: Mutex<SessionSnapshot>,
    pub store: Arc<Mutex<RequestStore>>,
    pub queues: Mutex<RequestQueues>,
    options: Mutex<SessionOptions>,
    pub dc: Arc<Dcenter>,
    pub environment: Arc<dyn SessionEnvironment>,
    pub shifted_dc_id: i32,
    owner_alive: AtomicBool,
    next_connection: AtomicU64,
    creating: Mutex<CreatingKeyType>,
    main: mpsc::UnboundedSender<MainCommand>,
}
impl SessionData {
    pub fn new(
        shifted_dc_id: i32,
        dc: Arc<Dcenter>,
        environment: Arc<dyn SessionEnvironment>,
        store: Arc<Mutex<RequestStore>>,
        main: mpsc::UnboundedSender<MainCommand>,
    ) -> Arc<Self> {
        Arc::new(Self {
            snapshot: Mutex::new(SessionSnapshot::default()),
            store,
            queues: Mutex::new(RequestQueues::default()),
            options: Mutex::new(SessionOptions::default()),
            dc,
            environment,
            shifted_dc_id,
            owner_alive: AtomicBool::new(true),
            next_connection: AtomicU64::new(0),
            creating: Mutex::new(CreatingKeyType::None),
            main,
        })
    }
    pub(crate) fn next_connection_id(&self) -> u64 {
        self.next_connection
            .fetch_add(1, Ordering::Relaxed)
            .checked_add(1)
            .expect("connection identifier exhausted")
    }
    fn with_session(&self, command: MainCommand) {
        if self.owner_alive.load(Ordering::Acquire) {
            let _ = self.main.send(command);
        }
    }
    pub(crate) fn queue_instance(&self, event: InstanceEvent) {
        self.with_session(MainCommand::Instance(event));
    }
    pub fn set_options(&self, options: SessionOptions) {
        *self.options.lock().unwrap() = options;
    }
    pub fn options(&self) -> SessionOptions {
        self.options.lock().unwrap().clone()
    }
    pub fn notify_connection_inited(&self, _sent_options: &SessionOptions) {
        if self.owner_alive.load(Ordering::Acquire) {
            self.dc.set_connection_inited(true);
        }
    }
    pub fn queue_try_to_receive(&self) {
        self.with_session(MainCommand::TryToReceive)
    }
    pub fn queue_need_to_resume_and_send(&self) {
        self.with_session(MainCommand::NeedToResumeAndSend)
    }
    pub fn queue_connection_state_change(&self, state: i32) {
        self.with_session(MainCommand::ConnectionStateChange(state))
    }
    pub fn queue_reset_done(&self) {
        self.with_session(MainCommand::ResetDone)
    }
    pub fn queue_send_anything(&self, ms: i64) {
        self.with_session(MainCommand::SendAnything(ms))
    }
    pub fn connection_inited(&self) -> bool {
        self.owner_alive.load(Ordering::Acquire) && self.dc.connection_inited()
    }
    pub fn get_persistent_key(&self) -> Option<Arc<AuthKey>> {
        self.owner_alive
            .load(Ordering::Acquire)
            .then(|| self.dc.get_persistent_key())
            .flatten()
    }
    pub fn get_temporary_key(&self, kind: TemporaryKeyType) -> Option<Arc<AuthKey>> {
        self.owner_alive
            .load(Ordering::Acquire)
            .then(|| self.dc.get_temporary_key(kind))
            .flatten()
    }
    pub fn acquire_key_creation(&self, kind: crate::mtproto_dc_options::DcType) -> CreatingKeyType {
        if !self.owner_alive.load(Ordering::Acquire) {
            return CreatingKeyType::None;
        }
        let mut mine = self.creating.lock().unwrap();
        assert_eq!(*mine, CreatingKeyType::None);
        *mine = self.dc.acquire_key_creation(kind);
        *mine
    }
    pub fn release_key_creation_on_done(
        &self,
        temporary: Arc<AuthKey>,
        persistent: Arc<AuthKey>,
    ) -> bool {
        self.release_generic_key_creation_on_done(temporary, Some(persistent))
    }
    pub fn release_cdn_key_creation_on_done(&self, temporary: Arc<AuthKey>) -> bool {
        assert_eq!(
            *self.creating.lock().unwrap(),
            CreatingKeyType::TemporaryRegular
        );
        self.release_generic_key_creation_on_done(temporary, None)
    }
    fn release_generic_key_creation_on_done(
        &self,
        temporary: Arc<AuthKey>,
        persistent: Option<Arc<AuthKey>>,
    ) -> bool {
        if !self.owner_alive.load(Ordering::Acquire) {
            return false;
        }
        let kind = std::mem::replace(&mut *self.creating.lock().unwrap(), CreatingKeyType::None);
        assert_ne!(kind, CreatingKeyType::None);
        let ok = self
            .dc
            .release_key_creation_on_done(kind, temporary, persistent.clone());
        if ok {
            let event = if kind == CreatingKeyType::Persistent {
                InstanceEvent::DcPersistentKeyChanged(
                    self.dc.id(),
                    persistent.expect("persistent creation"),
                )
            } else {
                InstanceEvent::DcTemporaryKeyChanged(self.dc.id())
            };
            self.queue_instance(event);
        }
        ok
    }
    pub fn release_key_creation_on_fail(&self) {
        let kind = std::mem::replace(&mut *self.creating.lock().unwrap(), CreatingKeyType::None);
        if kind != CreatingKeyType::None {
            self.dc.release_key_creation_on_fail(kind);
        }
    }
    pub fn destroy_temporary_key(&self, id: u64) {
        if self.owner_alive.load(Ordering::Acquire) && self.dc.destroy_temporary_key(id) {
            self.queue_instance(InstanceEvent::DcTemporaryKeyChanged(self.dc.id()));
        }
    }
    pub fn detach(&self) {
        self.owner_alive.store(false, Ordering::Release);
    }
    pub fn refresh_options(&self) {
        let proxy = self.environment.proxy_settings();
        let kind = if proxy.enabled {
            proxy.selected.kind
        } else {
            Type::None
        };
        self.set_options(SessionOptions {
            system_lang_code: self.environment.system_lang_code(),
            cloud_lang_code: self.environment.cloud_lang_code(),
            lang_pack_name: self.environment.lang_pack_name(),
            proxy: if proxy.enabled {
                proxy.selected
            } else {
                ProxyData::default()
            },
            use_ipv4: true,
            use_ipv6: proxy.try_ipv6,
            use_http: !matches!(kind, Type::Mtproto | Type::Web),
            use_tcp: kind != Type::Http,
        });
    }
    pub fn register_request(&self, request: SerializedRequest) -> RequestHandle {
        self.store.lock().unwrap().insert(request)
    }
}
impl Drop for SessionData {
    fn drop(&mut self) {
        let kind = *self.creating.lock().unwrap();
        if kind != CreatingKeyType::None {
            self.dc.release_key_creation_on_fail(kind);
        }
    }
}

use crate::connection_abstract::{Clock, TransportError};
use crate::core_types::{Random, Time};
use crate::session_private::{
    ClientInfo, SessionClock, SessionPrivate, TransportAction, CONNECTED, CONNECTING, DISCONNECTED,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tg_base::timer::{Timer, TimerHost};
struct SharedClock<C>(Rc<C>);
impl<C: Clock> Clock for SharedClock<C> {
    fn now(&self) -> Time {
        self.0.now()
    }
    fn update_unix_time(&self, time: i32) {
        self.0.update_unix_time(time)
    }
    fn next_message_id(&self) -> u64 {
        self.0.next_message_id()
    }
}
impl<C: SessionClock> SessionClock for SharedClock<C> {
    fn local_unix_seconds(&self) -> i64 {
        self.0.local_unix_seconds()
    }
    fn update_time(&self, time: i32, force: bool) {
        self.0.update_time(time, force)
    }
}
struct SharedRandom<R>(Rc<RefCell<R>>);
impl<R: Random> Random for SharedRandom<R> {
    fn fill(&mut self, bytes: &mut [u8]) {
        self.0.borrow_mut().fill(bytes)
    }
}
#[derive(Clone, Debug)]
pub enum SessionCommand {
    ClientInfo(ClientInfo),
    Send {
        handle: RequestHandle,
        can_wait: i64,
    },
    Cancel {
        request_id: i32,
        msg_id: u64,
    },
    SendAnything(i64),
    Restart,
    Stop,
    Kill,
    Ping,
    Unpaused,
    PauseDelivery(bool),
    ReInitConnection,
    Wake,
}
#[derive(Debug)]
pub enum TransportEvent {
    ConnectTimeout {
        id: u64,
        full_timeout: i64,
    },
    Connected {
        id: u64,
        tag: String,
        full_timeout: i64,
    },
    ProbeConnected {
        id: u64,
        tag: String,
        full_timeout: i64,
        ping_time: i64,
    },
    Disconnected(u64),
    Error(u64, i32),
    Packet {
        id: u64,
        words: tg_tl::Buffer,
        need_http_wait: bool,
    },
    ReceivedSome(u64),
    SyncTimeRequest(u64),
}
/// The executor owns sockets and queues writes separately from reading. submit
/// never awaits I/O; next_event is cancellation safe. This permits input while
/// a write is blocked and lets tests provide ordinary Tokio duplex streams.
#[allow(async_fn_in_trait)]
pub trait SessionTransport {
    fn submit(&mut self, action: TransportAction) -> Result<(), TransportError>;
    async fn next_event(&mut self) -> Result<TransportEvent, TransportError>;
}
#[derive(Clone)]
pub struct SessionHandle {
    data: Arc<SessionData>,
    commands: mpsc::UnboundedSender<SessionCommand>,
    wakeup: Arc<tokio::sync::Notify>,
}
impl SessionHandle {
    pub fn data(&self) -> &Arc<SessionData> {
        &self.data
    }
    /// Public MTProto session/key identifiers, never key bytes.
    pub fn identity(&self) -> (u64, u64) {
        self.data.snapshot.lock().unwrap().identity
    }
    pub fn endpoint(&self) -> Option<crate::mtproto_dc_options::Endpoint> {
        self.data.snapshot.lock().unwrap().endpoint.clone()
    }
    pub fn init_prefix(&self) -> Vec<i32> {
        self.data.snapshot.lock().unwrap().init_prefix.clone()
    }
    pub fn get_state_at(&self, now: i64) -> i32 {
        let snapshot = self.data.snapshot.lock().unwrap();
        if !snapshot.private_alive {
            return DISCONNECTED;
        }
        if snapshot.state < 0 {
            if let Some(at) = snapshot.retry_at {
                return (now - at).min(-1) as i32;
            }
        }
        snapshot.state
    }
    pub fn get_state(&self) -> i32 {
        self.get_state_at(tg_runtime::now())
    }
    pub fn request_state_at(&self, id: i32, now: i64) -> i32 {
        if !self.data.snapshot.lock().unwrap().private_alive {
            return 0;
        }
        let state = self.get_state_at(now);
        if state != CONNECTED {
            return if matches!(state, CONNECTING | DISCONNECTED) {
                1
            } else {
                state
            };
        }
        if id != 0 && self.data.queues.lock().unwrap().to_send.contains_key(&id) {
            2
        } else {
            0
        }
    }
    pub fn request_state(&self, id: i32) -> i32 {
        self.request_state_at(id, tg_runtime::now())
    }
    pub fn transport(&self) -> String {
        let snapshot = self.data.snapshot.lock().unwrap();
        if snapshot.private_alive && snapshot.state >= 0 {
            snapshot.transport.clone()
        } else {
            String::new()
        }
    }

    pub fn send(&self, command: SessionCommand) -> bool {
        self.commands.send(command).is_ok()
    }
    pub fn wake(&self) {
        self.wakeup.notify_one();
    }
}
pub struct SessionParameters<C, R> {
    pub clock: C,
    pub random: R,
    pub host: Rc<TimerHost>,
    pub info: ClientInfo,
}
pub struct Session<C: SessionClock, R: Random> {
    pub data: Arc<SessionData>,
    clock: Rc<C>,
    random: Rc<RefCell<R>>,
    info: ClientInfo,
    host: Rc<TimerHost>,
    private: Option<SessionPrivate<SharedClock<C>, SharedRandom<R>>>,
    protocol_policy: Option<(bool, bool, bool)>,
    pending_actions: VecDeque<TransportAction>,
    main: mpsc::UnboundedReceiver<MainCommand>,
    commands: mpsc::UnboundedReceiver<SessionCommand>,
    changes: mpsc::UnboundedReceiver<EnvironmentChange>,
    changes_open: bool,
    sender: Timer,
    sender_due: Rc<Cell<bool>>,
    wakeup: Arc<tokio::sync::Notify>,
    killed: bool,
    need_receive: bool,
    paused: bool,
    ms_send_call: i64,
    ms_wait: i64,
    ping: bool,
}
impl<C: SessionClock + 'static, R: Random + 'static> Session<C, R> {
    pub fn new(
        id: i32,
        dc: Arc<Dcenter>,
        environment: Arc<dyn SessionEnvironment>,
        store: Arc<Mutex<RequestStore>>,
        parameters: SessionParameters<C, R>,
    ) -> (Self, SessionHandle) {
        let SessionParameters {
            clock,
            random,
            host,
            info,
        } = parameters;
        let (main_tx, main) = mpsc::unbounded_channel();
        let (command_tx, commands) = mpsc::unbounded_channel();
        let changes = environment.subscribe();
        let data = SessionData::new(id, dc, environment, store, main_tx);
        let wakeup = Arc::new(tokio::sync::Notify::new());
        let due = Rc::new(Cell::new(false));
        let callback_due = due.clone();
        let callback_wake = wakeup.clone();
        let sender = Timer::in_host(
            &host,
            Some(Rc::new(move || {
                callback_due.set(true);
                callback_wake.notify_one();
            })),
        );
        let adjust_wake = wakeup.clone();
        host.set_adjust_waker(Some(Arc::new(move || adjust_wake.notify_one())));
        let mut session = Self {
            data,
            clock: Rc::new(clock),
            random: Rc::new(RefCell::new(random)),
            info,
            host,
            private: None,
            protocol_policy: None,
            pending_actions: VecDeque::new(),
            main,
            commands,
            changes,
            changes_open: true,
            sender,
            sender_due: due,
            wakeup: wakeup.clone(),
            killed: false,
            need_receive: false,
            paused: false,
            ms_send_call: 0,
            ms_wait: 0,
            ping: false,
        };
        session.refresh_options();
        session.start();
        let handle_data = session.data.clone();
        (
            session,
            SessionHandle {
                data: handle_data,
                commands: command_tx,
                wakeup,
            },
        )
    }
    pub fn set_alarm(&self, alarm: Rc<dyn tg_base::timer::Alarm>) {
        self.host.set_alarm(Some(alarm));
    }
    pub fn wakeup(&self) -> Arc<tokio::sync::Notify> {
        self.wakeup.clone()
    }
    pub fn start(&mut self) {
        self.kill_connection();
        self.private = Some(SessionPrivate::new(
            self.data.clone(),
            SharedClock(self.clock.clone()),
            SharedRandom(self.random.clone()),
            self.host.clone(),
            self.info.clone(),
        ));
    }
    pub fn restart(&mut self) {
        if self.killed {
            return;
        }
        self.refresh_options();
        if let Some(private) = &mut self.private {
            private.restart_now();
        }
    }
    pub fn refresh_options(&self) {
        self.data.refresh_options();
        if let Some((tcp, http, ipv6)) = self.protocol_policy {
            let mut options = self.data.options();
            options.use_tcp &= tcp;
            options.use_http &= http;
            options.use_ipv6 &= ipv6;
            self.data.set_options(options);
        }
    }
    pub fn set_protocol_policy(&mut self, tcp: bool, http: bool, ipv6: bool) {
        self.protocol_policy = Some((tcp, http, ipv6));
        self.re_init_connection();
    }
    pub fn set_connection_not_inited(&self) {
        self.data.dc.set_connection_inited(false);
    }
    pub fn re_init_connection(&mut self) {
        self.set_connection_not_inited();
        self.restart();
    }
    pub fn stop(&mut self) {
        if !self.killed {
            self.kill_connection();
        }
    }
    pub fn kill(&mut self) {
        self.stop();
        self.killed = true;
        self.sender.cancel();
        self.data.detach();
    }
    fn kill_connection(&mut self) {
        if let Some(mut private) = self.private.take() {
            private.do_disconnect();
            self.pending_actions.extend(private.take_actions());
        }
    }
    pub fn unpaused(&mut self) {
        if self.need_receive {
            self.need_receive = false;
            self.data.queue_try_to_receive();
        }
    }
    pub fn send_anything(&mut self, can_wait: i64) {
        if self.killed {
            return;
        }
        let now = self.host.now();
        self.ms_wait = if self.ms_send_call != 0 {
            if now > self.ms_send_call + self.ms_wait {
                0
            } else {
                (self.ms_send_call + self.ms_wait - now).min(can_wait)
            }
        } else {
            can_wait
        };
        if self.ms_wait != 0 {
            self.ms_send_call = now;
            self.sender.call_once(self.ms_wait);
        } else {
            self.sender.cancel();
            self.ms_send_call = 0;
            self.need_to_resume_and_send();
        }
    }
    pub fn need_to_resume_and_send(&mut self) {
        if self.killed {
            return;
        }
        if self.private.is_none() {
            self.start();
        }
        let ping = std::mem::take(&mut self.ping);
        let private = self.private.as_mut().unwrap();
        if ping {
            private.send_ping_force();
        } else {
            private.try_to_send();
        }
    }
    pub fn connection_state_change(&self, state: i32) {
        emit_on_main(
            self.data.environment.clone(),
            InstanceEvent::StateChange(self.data.shifted_dc_id, state),
        );
    }
    pub fn reset_done(&self) {
        emit_on_main(
            self.data.environment.clone(),
            InstanceEvent::SessionReset(self.data.shifted_dc_id),
        );
    }
    pub fn cancel(&self, request_id: i32, msg_id: u64) {
        let mut queues = self.data.queues.lock().unwrap();
        if request_id != 0 {
            queues.to_send.remove(&request_id);
        }
        if msg_id != 0 {
            queues.have_sent.remove(&msg_id);
        }
    }
    pub fn ping(&mut self) {
        self.ping = true;
        self.send_anything(0);
    }
    pub fn get_dc_with_shift(&self) -> i32 {
        self.data.shifted_dc_id
    }
    pub fn get_persistent_key(&self) -> Option<Arc<AuthKey>> {
        self.data.get_persistent_key()
    }
    pub fn get_temporary_key(&self, kind: TemporaryKeyType) -> Option<Arc<AuthKey>> {
        self.data.get_temporary_key(kind)
    }
    pub fn connection_inited(&self) -> bool {
        self.data.connection_inited()
    }
    pub fn acquire_key_creation(&self, kind: crate::mtproto_dc_options::DcType) -> CreatingKeyType {
        self.data.acquire_key_creation(kind)
    }
    pub fn release_key_creation_on_done(
        &self,
        key: Arc<AuthKey>,
        persistent: Arc<AuthKey>,
    ) -> bool {
        self.data.release_key_creation_on_done(key, persistent)
    }
    pub fn release_cdn_key_creation_on_done(&self, key: Arc<AuthKey>) -> bool {
        self.data.release_cdn_key_creation_on_done(key)
    }
    pub fn release_key_creation_on_fail(&self) {
        self.data.release_key_creation_on_fail();
    }
    pub fn notify_dc_connection_inited(&self) {
        self.data.dc.set_connection_inited(true);
    }
    pub fn destroy_temporary_key(&self, id: u64) {
        self.data.destroy_temporary_key(id);
    }
    pub fn request_state(&self, id: i32) -> i32 {
        if self.private.is_none() {
            return 0;
        }
        let state = self.get_state();
        if state != CONNECTED {
            return if matches!(state, CONNECTING | DISCONNECTED) {
                1
            } else {
                state
            };
        }
        if id != 0 && self.data.queues.lock().unwrap().to_send.contains_key(&id) {
            2
        } else {
            0
        }
    }
    pub fn get_state(&self) -> i32 {
        self.private
            .as_ref()
            .map_or(DISCONNECTED, |p| p.get_state())
    }
    pub fn transport(&self) -> String {
        self.private
            .as_ref()
            .map(|p| p.transport())
            .unwrap_or_default()
    }
    pub fn send_prepared(&self, handle: RequestHandle, can_wait: i64) {
        let id = {
            let mut store = self.data.store.lock().unwrap();
            let request = store.get_mut(handle).expect("owned request");
            request.set_msg_id(0);
            request.set_seq_no(0);
            request.request_id
        };
        self.data
            .queues
            .lock()
            .unwrap()
            .to_send
            .entry(id)
            .or_insert(handle);
        if can_wait >= 0 {
            self.data.queue_send_anything(can_wait);
        }
    }
    pub fn try_to_receive(&mut self) {
        if self.killed {
            return;
        }
        if self.paused || crate::facade::paused() {
            self.need_receive = true;
            return;
        }
        let main =
            self.data.shifted_dc_id == crate::core_types::bare_dc_id(self.data.shifted_dc_id);
        loop {
            let messages = std::mem::take(&mut self.data.queues.lock().unwrap().received);
            if messages.is_empty() {
                break;
            }
            for message in messages {
                if message.request_id != 0 {
                    emit_on_main(
                        self.data.environment.clone(),
                        InstanceEvent::Callback(message),
                    );
                } else if main {
                    emit_on_main(
                        self.data.environment.clone(),
                        InstanceEvent::Update(message),
                    );
                }
            }
        }
    }
    fn watch_dc_key_changes(&mut self, id: i32) {
        if id == self.data.shifted_dc_id
            || id == crate::core_types::bare_dc_id(self.data.shifted_dc_id)
        {
            if let Some(private) = &mut self.private {
                private.update_auth_key();
            }
        }
    }
    fn watch_dc_options_changes(&mut self, change: EnvironmentChange) {
        let Some(private) = &mut self.private else {
            return;
        };
        match change {
            EnvironmentChange::DcOptions(id)
                if crate::core_types::bare_dc_id(self.data.shifted_dc_id) == id =>
            {
                private.dc_options_changed()
            }
            EnvironmentChange::CdnConfig
                if self
                    .data
                    .environment
                    .dc_options()
                    .dc_type(self.data.shifted_dc_id)
                    == crate::mtproto_dc_options::DcType::Cdn =>
            {
                private.cdn_config_changed()
            }
            _ => {}
        }
    }
    pub fn on_transport_event(&mut self, event: TransportEvent) {
        let Some(private) = &mut self.private else {
            return;
        };
        match event {
            TransportEvent::ConnectTimeout { id, full_timeout } => {
                private.update_connect_timeout(id, full_timeout)
            }
            TransportEvent::Connected {
                id,
                tag,
                full_timeout,
            }
            | TransportEvent::ProbeConnected {
                id,
                tag,
                full_timeout,
                ..
            } => private.on_connected(id, true, tag, full_timeout),
            TransportEvent::Disconnected(id) => private.on_disconnected(id),
            TransportEvent::Error(id, code) => private.on_error(id, code),
            TransportEvent::Packet {
                id,
                words,
                need_http_wait,
            } => private.on_packet(id, words, need_http_wait),
            TransportEvent::ReceivedSome(id) => {
                if private.owns_connection(id) {
                    private.on_received_some();
                }
            }
            TransportEvent::SyncTimeRequest(id) => {
                if private.owns_connection(id) {
                    private.sync_time_request();
                }
            }
        }
    }
    pub fn process(&mut self) {
        if let Some(private) = &mut self.private {
            private.process_timers();
        } else {
            self.host.process();
        }
        if self.sender_due.replace(false) {
            self.need_to_resume_and_send();
        }
        while let Ok(command) = self.commands.try_recv() {
            match command {
                SessionCommand::Send { handle, can_wait } => self.send_prepared(handle, can_wait),
                SessionCommand::Cancel { request_id, msg_id } => self.cancel(request_id, msg_id),
                SessionCommand::SendAnything(wait) => self.send_anything(wait),
                SessionCommand::Restart => self.restart(),
                SessionCommand::Stop => self.stop(),
                SessionCommand::Kill => self.kill(),
                SessionCommand::Ping => self.ping(),
                SessionCommand::ReInitConnection => self.re_init_connection(),
                SessionCommand::Unpaused => {
                    self.paused = false;
                    self.unpaused();
                }
                SessionCommand::PauseDelivery(paused) => self.paused = paused,
                SessionCommand::ClientInfo(info) => {
                    self.info = info.clone();
                    if let Some(private) = &mut self.private {
                        private.info = info;
                    }
                }
                SessionCommand::Wake => {}
            }
        }
        while let Ok(change) = self.changes.try_recv() {
            match change {
                EnvironmentChange::TemporaryKey(id) => self.watch_dc_key_changes(id),
                change => self.watch_dc_options_changes(change),
            }
        }
        while let Ok(command) = self.main.try_recv() {
            match command {
                MainCommand::Instance(event) => emit_on_main(self.data.environment.clone(), event),
                MainCommand::TryToReceive => self.try_to_receive(),
                MainCommand::NeedToResumeAndSend => self.need_to_resume_and_send(),
                MainCommand::ConnectionStateChange(state) => self.connection_state_change(state),
                MainCommand::ResetDone => self.reset_done(),
                MainCommand::SendAnything(wait) => self.send_anything(wait),
            }
        }
    }
    pub fn take_transport_actions(&mut self) -> VecDeque<TransportAction> {
        let mut actions = std::mem::take(&mut self.pending_actions);
        if let Some(private) = &mut self.private {
            actions.extend(private.take_actions());
        }
        actions
    }
    pub async fn run(mut self, mut transport: impl SessionTransport) -> Result<(), TransportError> {
        loop {
            self.process();
            for action in self.take_transport_actions() {
                transport.submit(action)?;
            }
            if self.killed {
                return Ok(());
            }
            let wake = self.wakeup.clone();
            tokio::select! {event=transport.next_event()=>match event{Ok(event)=>self.on_transport_event(event),Err(error)=>{if let Some(private)=&mut self.private{private.restart();}return Err(error);}},command=self.commands.recv()=>match command{Some(command)=>self.handle_run_command(command),None=>{self.kill();return Ok(());}},change=self.changes.recv(),if self.changes_open=>match change{Some(EnvironmentChange::TemporaryKey(id))=>self.watch_dc_key_changes(id),Some(change)=>self.watch_dc_options_changes(change),None=>self.changes_open=false},_=wake.notified()=>{}}
        }
    }
    fn handle_run_command(&mut self, command: SessionCommand) {
        match command {
            SessionCommand::Send { handle, can_wait } => self.send_prepared(handle, can_wait),
            SessionCommand::Cancel { request_id, msg_id } => self.cancel(request_id, msg_id),
            SessionCommand::SendAnything(wait) => self.send_anything(wait),
            SessionCommand::Restart => self.restart(),
            SessionCommand::Stop => self.stop(),
            SessionCommand::Kill => self.kill(),
            SessionCommand::Ping => self.ping(),
            SessionCommand::ReInitConnection => self.re_init_connection(),
            SessionCommand::Unpaused => {
                self.paused = false;
                self.unpaused();
            }
            SessionCommand::PauseDelivery(paused) => self.paused = paused,
            SessionCommand::ClientInfo(info) => {
                self.info = info.clone();
                if let Some(private) = &mut self.private {
                    private.info = info;
                }
            }
            SessionCommand::Wake => {}
        }
    }
}
impl<C: SessionClock, R: Random> Drop for Session<C, R> {
    fn drop(&mut self) {
        self.private = None;
        self.data.detach();
    }
}
