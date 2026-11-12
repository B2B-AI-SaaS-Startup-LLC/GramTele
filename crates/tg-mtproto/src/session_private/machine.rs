use super::{decrypt_message, encrypt_message, Direction, Message};
use crate::{
    connection_abstract::{parse_not_secure_response, prepare_not_secure_packet, Clock},
    core_types::*,
    details::{
        mtproto_bound_key_creator::{is_destroyed_temporary_key_error, BoundKeyCreator},
        mtproto_dc_key_binder::DcKeyBindState,
        mtproto_dc_key_creator::{CreationStep, DcKeyError, DcKeyRequest, DcKeyResult},
        mtproto_dcenter::{temporary_key_type_by_dc_type, CreatingKeyType, TemporaryKeyType},
        mtproto_received_ids_manager::{ReceivedIdsManager, Result as IdResult, State as IdState},
        mtproto_serialized_request::{RequestHandle, SerializedRequest},
    },
    mtproto_auth_key::AuthKey,
    mtproto_dc_options::{DcType, Endpoint},
    mtproto_proxy_data::Type,
    mtproto_response::Response,
    session::{InstanceEvent, SessionData, SessionOptions},
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    rc::Rc,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
};
use tg_base::timer::{Timer, TimerHost};
pub const DISCONNECTED: i32 = 0;
pub const CONNECTING: i32 = 1;
pub const CONNECTED: i32 = 2;
static SYNC_TIME_REQUEST_DURATION: AtomicI64 = AtomicI64::new(500);
pub trait SessionClock: Clock {
    fn update_time(&self, server_time: i32, force: bool);
    fn local_unix_seconds(&self) -> i64;
}
impl SessionClock for crate::connection_abstract::RuntimeClock {
    fn update_time(&self, time: i32, force: bool) {
        tg_base::unixtime::update(time, force);
    }
    fn local_unix_seconds(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |time| time.as_secs() as i64)
    }
}
#[derive(Clone, Debug)]
pub struct ClientInfo {
    pub api_id: i32,
    pub app_version: String,
    pub local_utc_offset: i32,
    pub prefer_ipv6: bool,
}
impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            api_id: 17349,
            app_version: compute_app_version(
                "7.2.9",
                match std::env::consts::ARCH {
                    "aarch64" => "arm64",
                    "x86" => "i386",
                    architecture => architecture,
                },
                cfg!(windows),
                AppDistribution::Plain,
            ),
            local_utc_offset: 0,
            prefer_ipv6: false,
        }
    }
}
#[derive(Clone, Debug)]
pub struct ConnectionSpec {
    pub id: u64,
    pub endpoint: Endpoint,
    pub http: bool,
    pub priority: i32,
    pub protocol_dc_id: i16,
    pub for_files: bool,
    pub proxy: crate::mtproto_proxy_data::ProxyData,
}
struct Candidate {
    spec: ConnectionSpec,
    connected: bool,
    full_timeout: i64,
    tag: String,
    sent_key: u64,
}
#[derive(Debug)]
pub enum TransportAction {
    Connect(ConnectionSpec),
    Send(u64, Buffer),
    Close(u64),
    TimedOut(u64),
}
#[derive(Clone, Copy, Debug)]
enum AlarmKind {
    Retry,
    Old,
    Connected,
    Received,
    Better,
    Ping,
    CheckSent,
    ClearContainers,
}
struct Timers {
    retry: Timer,
    old: Timer,
    connected: Timer,
    received: Timer,
    better: Timer,
    ping: Timer,
    check: Timer,
    clear: Timer,
}
impl Timers {
    fn new(host: &Rc<TimerHost>, queue: Rc<RefCell<VecDeque<AlarmKind>>>) -> Self {
        let timer = |kind| {
            let queue = queue.clone();
            Timer::in_host(
                host,
                Some(Rc::new(move || queue.borrow_mut().push_back(kind))),
            )
        };
        Self {
            retry: timer(AlarmKind::Retry),
            old: timer(AlarmKind::Old),
            connected: timer(AlarmKind::Connected),
            received: timer(AlarmKind::Received),
            better: timer(AlarmKind::Better),
            ping: timer(AlarmKind::Ping),
            check: timer(AlarmKind::CheckSent),
            clear: timer(AlarmKind::ClearContainers),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandleResult {
    Success,
    Ignored,
    RestartConnection,
    ResetSession,
    DestroyTemporaryKey,
    ParseError,
}
#[derive(Clone, Copy)]
struct OuterInfo {
    outer_msg_id: u64,
    server_salt: u64,
    server_time: i32,
    bad_time: bool,
}
struct SentContainer {
    sent: i64,
    messages: Vec<u64>,
}
pub struct SessionPrivate<C: SessionClock, R: Random> {
    pub data: Arc<SessionData>,
    pub clock: C,
    pub random: R,
    pub info: ClientInfo,
    host: Rc<TimerHost>,
    timer_events: Rc<RefCell<VecDeque<AlarmKind>>>,
    timers: Timers,
    state: i32,
    retry_timeout: i64,
    retry_finish: i64,
    wait_received: i64,
    wait_connected: i64,
    first_sent_at: i64,
    old_connection: bool,
    started_connecting_at: i64,
    candidates: BTreeMap<u64, Candidate>,
    connection: Option<Candidate>,
    actions: VecDeque<TransportAction>,
    options: SessionOptions,
    real_dc_type: DcType,
    current_dc_type: DcType,
    key: Option<Arc<AuthKey>>,
    key_id: u64,
    session_id: u64,
    salt: u64,
    counter: u32,
    session_started: bool,
    need_session_reset: bool,
    ack: Vec<u64>,
    resend_ids: Vec<u64>,
    state_ids: BTreeSet<u64>,
    received: ReceivedIdsManager,
    resending: BTreeMap<u64, i32>,
    acked: BTreeMap<u64, i32>,
    state_requests: BTreeMap<u64, RequestHandle>,
    containers: BTreeMap<u64, SentContainer>,
    ping_id: u64,
    ping_to_send: u64,
    ping_send_at: i64,
    ping_msg_id: u64,
    bind_msg_id: u64,
    bind_sent: i64,
    key_creator: Option<BoundKeyCreator>,
}
impl<C: SessionClock, R: Random> SessionPrivate<C, R> {
    pub fn new(
        data: Arc<SessionData>,
        clock: C,
        random: R,
        host: Rc<TimerHost>,
        info: ClientInfo,
    ) -> Self {
        assert_ne!(data.shifted_dc_id, 0);
        data.snapshot.lock().unwrap().private_alive = true;
        let real = data.environment.dc_options().dc_type(data.shifted_dc_id);
        let queue = Rc::new(RefCell::new(VecDeque::new()));
        let timers = Timers::new(&host, queue.clone());
        let mut result = Self {
            options: data.options(),
            data,
            clock,
            random,
            info,
            host,
            timer_events: queue,
            timers,
            state: DISCONNECTED,
            retry_timeout: 1,
            retry_finish: 0,
            wait_received: 4000,
            wait_connected: 1000,
            first_sent_at: -1,
            old_connection: true,
            started_connecting_at: 0,
            candidates: BTreeMap::new(),
            connection: None,
            actions: VecDeque::new(),
            real_dc_type: real,
            current_dc_type: real,
            key: None,
            key_id: 0,
            session_id: 0,
            salt: 0,
            counter: 0,
            session_started: false,
            need_session_reset: false,
            ack: vec![],
            resend_ids: vec![],
            state_ids: BTreeSet::new(),
            received: ReceivedIdsManager::default(),
            resending: BTreeMap::new(),
            acked: BTreeMap::new(),
            state_requests: BTreeMap::new(),
            containers: BTreeMap::new(),
            ping_id: 0,
            ping_to_send: 0,
            ping_send_at: 0,
            ping_msg_id: 0,
            bind_msg_id: 0,
            bind_sent: 0,
            key_creator: None,
        };
        result.timers.clear.call_each(600000);
        result.connect_to_server(false);
        result
    }
    pub fn take_actions(&mut self) -> VecDeque<TransportAction> {
        std::mem::take(&mut self.actions)
    }
    pub fn process_timers(&mut self) {
        self.host.process();
        loop {
            let event = self.timer_events.borrow_mut().pop_front();
            let Some(event) = event else { break };
            match event {
                AlarmKind::Retry => self.retry_by_timer(),
                AlarmKind::Old => self.mark_connection_old(),
                AlarmKind::Connected => self.wait_connected_failed(),
                AlarmKind::Received => self.wait_received_failed(),
                AlarmKind::Better => self.wait_better_failed(),
                AlarmKind::Ping => self.send_ping_by_timer(),
                AlarmKind::CheckSent => self.check_sent_requests(),
                AlarmKind::ClearContainers => self.clear_old_containers(),
            }
        }
        self.data.snapshot.lock().unwrap().retry_at = if self.timers.retry.is_active() {
            Some(self.retry_finish)
        } else {
            None
        };
    }
    pub fn next_deadline(&self) -> Option<i64> {
        self.host.next_deadline()
    }
    pub fn get_shifted_dc_id(&self) -> i32 {
        self.data.shifted_dc_id
    }
    pub fn get_state(&self) -> i32 {
        if self.state < 0 && self.timers.retry.is_active() {
            (self.host.now() - self.retry_finish).min(-1) as i32
        } else {
            self.state
        }
    }
    pub fn transport(&self) -> String {
        if self.state < 0 {
            String::new()
        } else {
            self.connection
                .as_ref()
                .map(|c| c.tag.clone())
                .unwrap_or_default()
        }
    }
    pub fn session_id(&self) -> u64 {
        self.session_id
    }
    pub fn server_salt(&self) -> u64 {
        self.salt
    }
    fn notify(&self, event: InstanceEvent) {
        self.data.queue_instance(event)
    }
    fn set_state(&mut self, state: i32, only: Option<i32>) -> bool {
        if only.is_some_and(|s| self.state != s) || self.state == state {
            return false;
        }
        self.state = state;
        {
            let mut snapshot = self.data.snapshot.lock().unwrap();
            snapshot.state = state;
            snapshot.retry_at = if state < 0 {
                Some(self.host.now() - (state as i64))
            } else {
                None
            };
        }
        if state < 0 {
            self.retry_timeout = -(state as i64);
            self.timers.retry.call_once(self.retry_timeout);
            self.retry_finish = self.host.now() + self.retry_timeout;
        }
        self.data.queue_connection_state_change(state);
        true
    }
    fn append_test_connection(&mut self, http: bool, endpoint: Endpoint) {
        let connection_id = self.data.next_connection_id();
        let priority = if endpoint.ip.parse::<std::net::Ipv6Addr>().is_ok() {
            if self.info.prefer_ipv6 {
                2
            } else {
                0
            }
        } else {
            1
        } + i32::from(!http)
            + i32::from(!endpoint.secret.is_empty());
        let spec = ConnectionSpec {
            id: connection_id,
            endpoint,
            http,
            priority,
            protocol_dc_id: self.get_protocol_dc_id(),
            for_files: is_media_cluster(self.data.shifted_dc_id)
                || self.real_dc_type == DcType::Cdn,
            proxy: self.options.proxy.clone(),
        };
        self.first_sent_at = 0;
        self.old_connection = false;
        self.timers.old.call_once(192000);
        self.actions
            .push_back(TransportAction::Connect(spec.clone()));
        let full_timeout = if spec.proxy.try_custom_resolve() {
            4000 * spec.proxy.resolved_ips.len().max(1) as i64
        } else {
            8000
        };
        self.candidates.insert(
            spec.id,
            Candidate {
                tag: if http { "HTTP" } else { "TCP" }.into(),
                spec,
                connected: false,
                full_timeout,
                sent_key: 0,
            },
        );
    }
    pub fn get_protocol_dc_id(&self) -> i16 {
        let dc = bare_dc_id(self.data.shifted_dc_id);
        let dc = if dc >= 1000 { dc - 1000 } else { dc };
        let tested = if self.data.environment.is_test_mode() {
            10000 + dc
        } else {
            dc
        };
        if self.current_dc_type == DcType::MediaCluster {
            (-tested) as i16
        } else {
            tested as i16
        }
    }
    fn real_dc_type_changed(&mut self) -> bool {
        let now = self
            .data
            .environment
            .dc_options()
            .dc_type(self.data.shifted_dc_id);
        let changed = now != self.real_dc_type;
        self.real_dc_type = now;
        changed
    }
    pub fn dc_options_changed(&mut self) {
        self.retry_timeout = 1;
        self.connect_to_server(true)
    }
    pub fn cdn_config_changed(&mut self) {
        self.connect_to_server(true)
    }
    pub fn connect_to_server(&mut self, after_config: bool) {
        if after_config && (!self.candidates.is_empty() || self.connection.is_some()) {
            return;
        }
        self.destroy_all_connections();
        if self.real_dc_type_changed() && self.key_creator.is_some() {
            self.destroy_temporary_key();
            return;
        }
        self.options = self.data.options();
        let bare = bare_dc_id(self.data.shifted_dc_id);
        self.current_dc_type = self.try_acquire_key_creation();
        let options = self.data.environment.dc_options();
        if self.current_dc_type == DcType::Cdn
            && !self.data.environment.is_keys_destroyer()
            && !options.has_cdn_keys_for_dc(bare)
        {
            self.request_cdn_config();
            return;
        }
        if matches!(self.options.proxy.kind, Type::Mtproto | Type::Web) {
            self.append_test_connection(
                false,
                Endpoint {
                    id: bare,
                    flags: 0,
                    ip: String::new(),
                    port: 0,
                    secret: vec![],
                },
            );
        } else {
            let special = self.current_dc_type == DcType::Temporary;
            let variants = options.lookup(
                bare,
                self.current_dc_type,
                self.options.proxy.kind != Type::None,
            );
            let v4 = special || self.options.use_ipv4;
            let v6 = !special && self.options.use_ipv6;
            let tcp = special || self.options.use_tcp;
            let http = !special && self.options.use_http;
            let skip_address = if !v4 {
                0
            } else if !v6 {
                1
            } else {
                2
            };
            let skip_protocol = if !tcp {
                0
            } else if !http {
                1
            } else {
                2
            };
            for (address, protocols) in variants.into_iter().enumerate() {
                if address == skip_address {
                    continue;
                }
                for (protocol, endpoints) in protocols.into_iter().enumerate() {
                    if protocol == skip_protocol {
                        continue;
                    }
                    for endpoint in endpoints {
                        self.append_test_connection(protocol == 1, endpoint);
                    }
                }
            }
        }
        if self.candidates.is_empty() {
            if self.data.environment.is_keys_destroyer() {
                self.notify(InstanceEvent::KeyWasPossiblyDestroyed(
                    self.data.shifted_dc_id,
                ));
            } else if after_config {
                self.restart();
            } else {
                self.notify(InstanceEvent::RequestConfig);
            }
            return;
        }
        let now = self.host.now();
        if self.started_connecting_at == 0 {
            self.started_connecting_at = now;
        } else if now - self.started_connecting_at > 8000 {
            self.notify(InstanceEvent::RequestConfigIfOld);
        }
        self.timers.retry.cancel();
        self.timers.connected.cancel();
        self.set_state(CONNECTING, None);
        self.bind_msg_id = 0;
        self.ping_id = 0;
        self.ping_msg_id = 0;
        self.ping_to_send = 0;
        self.ping_send_at = 0;
        self.timers.ping.cancel();
        self.timers.connected.call_once(self.wait_connected);
    }
    pub fn update_connect_timeout(&mut self, id: u64, timeout: i64) {
        if let Some(candidate) = self.candidates.get_mut(&id) {
            candidate.full_timeout = timeout;
        }
    }
    pub fn owns_connection(&self, id: u64) -> bool {
        self.candidates.contains_key(&id)
            || self.connection.as_ref().is_some_and(|c| c.spec.id == id)
    }
    pub fn on_connected(&mut self, id: u64, is_connected: bool, tag: String, full_timeout: i64) {
        if !self.candidates.contains_key(&id) {
            return;
        }
        if !is_connected {
            self.restart();
            return;
        }
        let Some(c) = self.candidates.get_mut(&id) else {
            return;
        };
        c.connected = true;
        c.tag = tag;
        c.full_timeout = full_timeout;
        let priority = c.spec.priority;
        self.wait_connected = 1000;
        self.timers.connected.cancel();
        if self.candidates.values().any(|c| c.spec.priority > priority) {
            self.timers.better.call_once(2000);
        } else {
            self.timers.better.cancel();
            self.connection = self.candidates.remove(&id);
            self.data.snapshot.lock().unwrap().transport =
                self.connection.as_ref().unwrap().tag.clone();
            self.close_candidates();
            self.check_auth_key();
        }
    }
    fn close_candidates(&mut self) {
        for (id, _) in std::mem::take(&mut self.candidates) {
            self.actions.push_back(TransportAction::Close(id));
        }
    }
    pub fn on_disconnected(&mut self, id: u64) {
        if !self.owns_connection(id) {
            return;
        }
        self.remove_test_connection(id);
        if self.candidates.is_empty() {
            self.destroy_all_connections();
            self.restart();
        } else {
            self.confirm_best_connection();
        }
    }
    fn remove_test_connection(&mut self, id: u64) {
        if self.candidates.remove(&id).is_some() {
            self.actions.push_back(TransportAction::Close(id));
        }
    }
    fn confirm_best_connection(&mut self) {
        if self.timers.better.is_active() {
            return;
        }
        let id = self
            .candidates
            .iter()
            .max_by_key(|(id, c)| {
                (
                    if c.connected { c.spec.priority } else { -1 },
                    std::cmp::Reverse(**id),
                )
            })
            .filter(|(_, c)| c.connected)
            .map(|(&id, _)| id);
        if let Some(id) = id {
            self.connection = self.candidates.remove(&id);
            self.data.snapshot.lock().unwrap().transport =
                self.connection.as_ref().unwrap().tag.clone();
            self.close_candidates();
            self.check_auth_key();
        }
    }
    fn destroy_all_connections(&mut self) {
        self.clear_unbound_key_creator();
        self.data.snapshot.lock().unwrap().transport.clear();
        self.timers.better.cancel();
        self.timers.received.cancel();
        self.timers.connected.cancel();
        self.close_candidates();
        if let Some(c) = self.connection.take() {
            self.actions.push_back(TransportAction::Close(c.spec.id));
        }
    }
    pub fn do_disconnect(&mut self) {
        self.destroy_all_connections();
        self.set_state(DISCONNECTED, None);
    }
    pub fn restart(&mut self) {
        self.timers.received.cancel();
        self.timers.connected.cancel();
        self.do_disconnect();
        if self.need_session_reset {
            self.reset_session();
        }
        if !self.timers.retry.is_active() {
            self.set_state(-(self.retry_timeout as i32), None);
        }
    }
    pub fn restart_now(&mut self) {
        self.retry_timeout = 1;
        self.timers.retry.cancel();
        self.restart();
    }
    fn retry_by_timer(&mut self) {
        self.retry_timeout = match self.retry_timeout {
            0..=2 => self.retry_timeout + 1,
            3 => 1000,
            4..64000 => self.retry_timeout * 2,
            n => n,
        };
        self.connect_to_server(false);
    }
    fn connecting_timed_out(&mut self) {
        for candidate in self.candidates.values() {
            if !candidate.spec.http && candidate.spec.endpoint.secret.first() == Some(&0xee) {
                self.sync_time_request();
            }
        }
        for &id in self.candidates.keys() {
            self.actions.push_back(TransportAction::TimedOut(id));
        }
        self.do_disconnect();
    }
    fn wait_connected_failed(&mut self) {
        let max = self
            .candidates
            .values()
            .map(|c| c.full_timeout)
            .max()
            .unwrap_or(8000)
            .max(8000);
        if self.wait_connected < max {
            self.wait_connected = (self.wait_connected * 2).min(max);
        }
        self.connecting_timed_out();
        self.connect_to_server(false);
    }
    fn wait_received_failed(&mut self) {
        self.wait_received = (self.wait_received * 2).min(64000);
        self.do_disconnect();
        if !self.timers.retry.is_active() {
            self.connect_to_server(false);
            self.notify(InstanceEvent::RestartedByTimeout(self.data.shifted_dc_id));
        }
    }
    fn wait_better_failed(&mut self) {
        self.confirm_best_connection();
    }
    pub fn on_sent_some(&mut self, size: u64) {
        if !self.timers.received.is_active() {
            let remain = if self.old_connection {
                self.wait_received
            } else {
                ((size * self.wait_received as u64 / 8192) as i64).clamp(self.wait_received, 64000)
            };
            self.timers.received.call_once(remain);
        }
        if self.first_sent_at == 0 {
            self.first_sent_at = self.host.now();
        }
    }
    pub fn on_received_some(&mut self) {
        self.old_connection = false;
        self.timers.old.call_once(192000);
        self.timers.received.cancel();
        if self.first_sent_at > 0 {
            let ms = self.host.now() - self.first_sent_at;
            if ms > 0 && ms * 2 < self.wait_received {
                self.wait_received = (ms * 2).max(4000);
            }
            self.first_sent_at = -1;
        }
    }
    fn mark_connection_old(&mut self) {
        self.old_connection = true;
        self.wait_received = 4000;
    }
    fn request_cdn_config(&self) {
        self.notify(InstanceEvent::RequestCdnConfig);
    }
    pub fn sync_time_request(&self) {
        self.notify(InstanceEvent::SyncHttpUnixtime);
    }
    fn change_session_id(&mut self) {
        let old = self.session_id;
        loop {
            self.session_id = self.random.u64();
            if self.session_id != old {
                break;
            }
        }
        self.counter = 0;
        self.session_started = false;
        self.ack.clear();
        self.resend_ids.clear();
        self.state_ids.clear();
        self.received.clear();
    }
    fn reset_session(&mut self) {
        self.need_session_reset = false;
        self.change_session_id();
        self.data.queue_reset_done();
    }
    fn next_request_seq_number(&mut self, ack: bool) -> u32 {
        let result = self.counter;
        self.counter = self.counter.wrapping_add(u32::from(ack));
        result.wrapping_mul(2).wrapping_add(u32::from(ack))
    }
    fn mark_session_as_started(&mut self) -> bool {
        !std::mem::replace(&mut self.session_started, true)
    }
    fn set_current_key_id(&mut self, key: u64) {
        if self.key_id != key {
            self.key_id = key;
            self.change_session_id();
        }
    }
    fn check_auth_key(&mut self) {
        if self.key_id != 0 {
            self.auth_key_checked();
        } else if self.data.environment.is_keys_destroyer() {
            self.apply_auth_key(self.data.get_persistent_key());
        } else {
            self.apply_auth_key(
                self.data
                    .get_temporary_key(temporary_key_type_by_dc_type(self.current_dc_type)),
            );
        }
    }
    pub fn update_auth_key(&mut self) {
        if !self.data.environment.is_keys_destroyer()
            && self.key_creator.is_none()
            && self.connection.is_some()
        {
            self.apply_auth_key(
                self.data
                    .get_temporary_key(temporary_key_type_by_dc_type(self.current_dc_type)),
            );
        }
    }
    fn no_media_key_with_existing_regular_key(&self) -> bool {
        temporary_key_type_by_dc_type(self.current_dc_type) == TemporaryKeyType::MediaCluster
            && self
                .data
                .get_temporary_key(TemporaryKeyType::Regular)
                .is_some()
    }
    fn try_acquire_key_creation(&mut self) -> DcType {
        if self.key_creator.is_some() {
            return self.current_dc_type;
        }
        if self.data.environment.is_keys_destroyer() {
            return self.real_dc_type;
        }
        let acquired = self.data.acquire_key_creation(self.real_dc_type);
        if acquired == CreatingKeyType::None {
            return self.real_dc_type;
        }
        self.key_creator = Some(BoundKeyCreator::new(DcKeyRequest {
            persistent_needed: acquired == CreatingKeyType::Persistent,
            temporary_expires_in: 86400,
        }));
        if self.real_dc_type == DcType::MediaCluster
            && acquired != CreatingKeyType::TemporaryMediaCluster
        {
            DcType::Regular
        } else {
            self.real_dc_type
        }
    }
    pub fn apply_auth_key(&mut self, key: Option<Arc<AuthKey>>) {
        self.key = key;
        let new = self.key.as_ref().map_or(0, |k| k.key_id());
        if self.key_id != 0 {
            if self.key_id == new {
                return;
            }
            self.set_current_key_id(0);
            if self.connection.is_some() {
                self.restart();
            }
            return;
        }
        if self.connection.is_none() {
            return;
        }
        self.set_current_key_id(new);
        if new != 0 {
            self.auth_key_checked();
            return;
        }
        if self.data.environment.is_keys_destroyer() {
            self.notify(InstanceEvent::KeyWasPossiblyDestroyed(
                self.data.shifted_dc_id,
            ));
        } else if self.no_media_key_with_existing_regular_key() {
            let kind = self.try_acquire_key_creation();
            if self.key_creator.is_some() && kind != self.current_dc_type {
                self.restart();
                return;
            }
        }
        let dc = bare_dc_id(self.data.shifted_dc_id);
        let protocol = self.get_protocol_dc_id();
        if let Some(creator) = &mut self.key_creator {
            match creator.start(dc, protocol, &mut self.random) {
                Ok(body) => self.send_unencrypted(body),
                Err(error) => self.key_creation_failed(error),
            }
        }
    }
    fn auth_key_checked(&mut self) {
        if self.salt != 0 && self.set_state(CONNECTED, None) {
            self.resend_all();
        }
        self.ping_to_send = self.random.u64();
        self.data.queue_need_to_resume_and_send();
    }
    fn send_unencrypted(&mut self, body: Buffer) {
        let request = body_request(body, self.host.now());
        let packet =
            prepare_not_secure_packet(&request, self.clock.next_message_id(), &mut self.random);
        if let Some(c) = &self.connection {
            let size = packet.len() * 4;
            self.actions
                .push_back(TransportAction::Send(c.spec.id, packet));
            self.on_sent_some(size as u64);
        }
    }
    fn key_creation_failed(&mut self, error: DcKeyError) {
        self.release_key_creation_on_fail();
        if error == DcKeyError::UnknownPublicKey && self.real_dc_type == DcType::Cdn {
            self.request_cdn_config();
        } else {
            self.restart();
        }
    }
    fn unbound_ready(&mut self, mut result: DcKeyResult) {
        self.salt = result.temporary_server_salt;
        let Some(key) = Arc::get_mut(&mut result.temporary_key) else {
            self.release_key_creation_on_fail();
            self.restart();
            return;
        };
        key.set_expires_at(self.clock.now().unix_seconds + 86430);
        if self.real_dc_type != DcType::Cdn {
            let persistent = result
                .persistent_key
                .or_else(|| self.data.get_persistent_key());
            let Some(persistent) = persistent else {
                self.release_key_creation_on_fail();
                self.restart();
                return;
            };
            self.key_creator
                .as_mut()
                .expect("creating")
                .bind(persistent);
        }
        self.apply_auth_key(Some(result.temporary_key));
        if self.real_dc_type == DcType::Cdn {
            self.key_creator = None;
            if !self
                .data
                .release_cdn_key_creation_on_done(self.key.as_ref().unwrap().clone())
            {
                self.restart();
            } else {
                self.data.queue_need_to_resume_and_send();
            }
        }
    }
    fn clear_unbound_key_creator(&mut self) {
        if let Some(creator) = &mut self.key_creator {
            creator.stop();
        }
    }
    fn release_key_creation_on_fail(&mut self) {
        if self.key_creator.take().is_some() {
            self.data.release_key_creation_on_fail();
        }
    }
    fn destroy_old_enough_persistent_key(&self) -> bool {
        let key = self
            .key_creator
            .as_ref()
            .expect("creating")
            .bind_persistent_key();
        let created = key.creation_time();
        if created > 0 && self.host.now() - created < 60000 {
            return false;
        }
        self.notify(InstanceEvent::KeyDestroyedOnServer(
            self.data.shifted_dc_id,
            key.key_id(),
        ));
        true
    }
    pub fn destroy_temporary_key(&mut self) {
        if self.data.environment.is_keys_destroyer() {
            self.notify(InstanceEvent::KeyWasPossiblyDestroyed(
                self.data.shifted_dc_id,
            ));
            return;
        }
        self.release_key_creation_on_fail();
        if let Some(key) = &self.key {
            self.data.destroy_temporary_key(key.key_id());
        }
        self.apply_auth_key(None);
        self.restart();
    }
    pub fn on_error(&mut self, id: u64, code: i32) {
        if !self.owns_connection(id) {
            return;
        }
        if code == -444 {
            self.notify(InstanceEvent::BadConfigurationError);
        }
        self.remove_test_connection(id);
        if self.candidates.is_empty() {
            self.handle_error(code);
        } else {
            self.confirm_best_connection();
        }
    }
    fn handle_error(&mut self, code: i32) {
        self.destroy_all_connections();
        self.timers.connected.cancel();
        if code == -404 {
            self.destroy_temporary_key();
        } else {
            self.restart();
        }
    }
    fn next_id(&self) -> u64 {
        self.clock.next_message_id()
    }
    fn prepare_to_send(&mut self, handle: RequestHandle, current: u64, force: bool) -> u64 {
        let data = self.data.clone();
        let mut store = data.store.lock().unwrap();
        let request = store.get_mut(handle).expect("owned request");
        let old = request.get_msg_id();
        if old != 0 {
            self.resending.remove(&old);
            drop(store);
            if force || old > current {
                self.replace_msg_id(handle, current)
            } else {
                old
            }
        } else {
            let ack = request.need_ack();
            let seq = self.next_request_seq_number(ack);
            request.set_msg_id(current);
            request.set_seq_no(seq);
            current
        }
    }
    fn replace_msg_id(&mut self, handle: RequestHandle, mut new: u64) -> u64 {
        let data = self.data.clone();
        let mut queues = data.queues.lock().unwrap();
        let mut store = data.store.lock().unwrap();
        let request = store.get_mut(handle).expect("owned request");
        let old = request.get_msg_id();
        if old == new {
            return new;
        }
        while self.resending.contains_key(&new)
            || self.acked.contains_key(&new)
            || queues.have_sent.contains_key(&new)
        {
            new = self.next_id();
        }
        if let Some(id) = self.resending.remove(&old) {
            self.resending.insert(new, id);
        }
        if let Some(id) = self.acked.remove(&old) {
            self.acked.insert(new, id);
        }
        if let Some(h) = queues.have_sent.remove(&old) {
            queues.have_sent.insert(new, h);
        }
        for container in self.containers.values_mut() {
            for id in &mut container.messages {
                if *id == old {
                    *id = new;
                }
            }
        }
        request.set_msg_id(new);
        let seq = self.next_request_seq_number(request.need_ack());
        request.set_seq_no(seq);
        new
    }
    fn place_to_container(
        &mut self,
        body: &mut Buffer,
        big: &mut u64,
        force: bool,
        handle: RequestHandle,
    ) {
        let id = self.prepare_to_send(handle, *big, force);
        if id >= *big {
            *big = self.next_id();
        }
        let store = self.data.store.lock().unwrap();
        let r = store.get(handle).unwrap();
        body.extend_from_slice(&r.words[4..4 + r.message_size()]);
    }
    pub fn prepare_init_params(&self) -> tg_scheme::JSONValue {
        let shift = self.clock.now().unix_seconds - self.clock.local_unix_seconds();
        let mut sliced = self.info.local_utc_offset as i64 - shift;
        while sliced < -12 * 3600 {
            sliced += 24 * 3600;
        }
        while sliced > 14 * 3600 {
            sliced -= 24 * 3600;
        }
        let rounded = ((sliced.abs() as f64 / 900.).round() as i32) * 900 * sliced.signum() as i32;
        tg_scheme::mtp_json_object(vec![tg_scheme::mtp_json_object_value(
            b"tz_offset".to_vec(),
            tg_scheme::mtp_json_number(rounded as f64),
        )])
    }
    fn init_prefix(&self) -> Buffer {
        let proxy = matches!(self.options.proxy.kind, Type::Mtproto | Type::Web);
        let proxy = proxy.then(|| {
            tg_scheme::mtp_input_client_proxy(
                self.options.proxy.host.as_bytes().to_vec(),
                self.options.proxy.port as i32,
            )
        });
        let model = if self.current_dc_type == DcType::Cdn {
            "n/a".into()
        } else {
            self.data.environment.device_model()
        };
        let system = if self.current_dc_type == DcType::Cdn {
            "n/a".into()
        } else {
            self.data.environment.system_version()
        };
        tg_tl::serialize(&tg_scheme::InvokeWithLayer::new(
            229,
            tg_scheme::InitConnection::new(
                tg_tl::Flags::from_raw(2 | u64::from(proxy.is_some())),
                self.info.api_id,
                model.into_bytes(),
                system.into_bytes(),
                self.info.app_version.as_bytes().to_vec(),
                self.options.system_lang_code.as_bytes().to_vec(),
                self.options.lang_pack_name.as_bytes().to_vec(),
                self.options.cloud_lang_code.as_bytes().to_vec(),
                proxy,
                Some(self.prepare_init_params()),
                SerializedRequest::default(),
            ),
        ))
    }
    fn wrap_invoke_after(&self, handle: RequestHandle, init: bool) -> Message {
        let queues = self.data.queues.lock().unwrap();
        let store = self.data.store.lock().unwrap();
        let request = store.get(handle).unwrap();
        let mut body = request.body().to_vec();
        if let Some(after) = request.after.and_then(|id| store.get(id)) {
            let id = after.get_msg_id();
            if id != 0 && queues.have_sent.contains_key(&id) {
                let mut wrapped = vec![0xcb9f372d_u32 as i32, id as i32, (id >> 32) as i32];
                wrapped.extend(body);
                body = wrapped;
            }
        }
        if init {
            let mut wrapped = self.init_prefix();
            wrapped.extend(body);
            body = wrapped;
        }
        Message {
            salt: self.salt,
            session_id: self.session_id,
            msg_id: request.get_msg_id(),
            seq_no: request.get_seq_no(),
            body,
        }
    }
    fn insert_service(&self, body: Buffer) -> RequestHandle {
        self.data
            .register_request(body_request(body, self.host.now()))
    }
    pub fn try_to_send(&mut self) {
        if self.connection.is_none() || self.key_id == 0 {
            return;
        }
        let first_ping = self.state != CONNECTED;
        let send_all = !first_ping && self.key_creator.is_none();
        let main = get_dc_id_shift(self.data.shifted_dc_id) == 0;
        if first_ping && self.ping_to_send == 0 {
            return;
        }
        if main
            && !first_ping
            && self.ping_to_send == 0
            && self.ping_id == 0
            && self.ping_send_at <= self.host.now()
        {
            self.ping_to_send = self.random.u64();
        }
        let force = send_all && self.mark_session_as_started();
        if force {
            if let Some(creator) = &mut self.key_creator {
                creator.restart_binder();
            }
        }
        let ping = if self.ping_to_send != 0 {
            let mut body = vec![
                if first_ping || !main {
                    0x7abe77ec_u32
                } else {
                    self.timers.ping.call_once(45000);
                    0xf3427b8c_u32
                } as i32,
                self.ping_to_send as i32,
                (self.ping_to_send >> 32) as i32,
            ];
            if !first_ping && main {
                body.push(60);
            }
            self.ping_send_at = self.host.now() + 30000;
            self.ping_id = std::mem::take(&mut self.ping_to_send);
            Some(self.insert_service(body))
        } else {
            None
        };
        let mut ack = None;
        let mut resend = None;
        let mut state = None;
        let mut http = None;
        let mut bind = None;
        if !first_ping {
            if !self.ack.is_empty() {
                let ids = std::mem::take(&mut self.ack);
                ack = Some(self.insert_service(id_vector(0x62d6b459, &ids)));
            }
            if !self.resend_ids.is_empty() {
                let ids = std::mem::take(&mut self.resend_ids);
                resend = Some(self.insert_service(id_vector(0x7d861a08, &ids)));
            }
            if !self.state_ids.is_empty() {
                let ids: Vec<_> = std::mem::take(&mut self.state_ids).into_iter().collect();
                state = Some(self.insert_service(id_vector(0xda69fb52, &ids)));
            }
            if self.connection.as_ref().is_some_and(|c| c.spec.http) {
                http = Some(self.insert_service(vec![0x9299359f_u32 as i32, 100, 30, 25000]));
            }
            if self.bind_msg_id == 0 && self.key_creator.as_ref().is_some_and(|c| c.ready_to_bind())
            {
                let mut request = self.key_creator.as_ref().unwrap().prepare_bind_request(
                    self.key.as_ref().unwrap(),
                    self.session_id,
                    self.next_id(),
                    self.host.now(),
                    &mut self.random,
                );
                request.set_seq_no(self.next_request_seq_number(request.need_ack()));
                bind = Some(self.data.register_request(request));
            }
        }
        let requests: Vec<_> = if send_all {
            let queues = self.data.queues.lock().unwrap();
            let store = self.data.store.lock().unwrap();
            let mut total = 0;
            queues
                .to_send
                .iter()
                .take_while(|(_, handle)| {
                    let take = total < 16384;
                    if take {
                        total += store.get(**handle).unwrap().words.len();
                    }
                    take
                })
                .map(|(&id, &handle)| (id, handle))
                .collect()
        } else {
            vec![]
        };
        let count = requests.len()
            + [ping, ack, resend, state, http, bind]
                .iter()
                .flatten()
                .count();
        if count == 0 {
            return;
        }
        let first = [ping, ack, resend, state, http, bind]
            .into_iter()
            .flatten()
            .next()
            .or_else(|| requests.first().map(|p| p.1))
            .unwrap();
        let container = count != 1
            || self
                .data
                .store
                .lock()
                .unwrap()
                .get(first)
                .unwrap()
                .force_send_in_container;
        let needs_layer = !self.data.connection_inited();
        let mut tracking = vec![];
        let mut check = false;
        let mut any = false;
        let mut service_handles = vec![];
        let mut source = None;
        let outgoing = if !container {
            self.prepare_to_send(first, self.next_id(), force && bind.is_none());
            let id = self
                .data
                .store
                .lock()
                .unwrap()
                .get(first)
                .unwrap()
                .get_msg_id();
            if bind.is_some() {
                self.bind_msg_id = id;
                self.bind_sent = self.host.now();
                any = true;
            } else if ping.is_some() {
                self.ping_msg_id = id;
                any = true;
            } else if state.is_some() || resend.is_some() {
                self.state_requests.insert(id, first);
                any = true;
            }
            let data = self.data.clone();
            let mut queues = data.queues.lock().unwrap();
            let mut store = data.store.lock().unwrap();
            let r = store.get_mut(first).unwrap();
            let init = needs_layer && r.needs_layer;
            let after = r.after.is_some();
            if r.request_id != 0 {
                if r.need_ack() {
                    r.last_sent_time = self.host.now();
                    queues.have_sent.entry(id).or_insert(first);
                    any = true;
                } else {
                    self.acked.entry(id).or_insert(r.request_id);
                }
            } else if state.is_none() && resend.is_none() {
                service_handles.push(first);
            }
            drop(store);
            drop(queues);
            let msg = self.wrap_invoke_after(first, init);
            if !init && !after {
                source = Some(first);
            }
            msg
        } else {
            let mut body = vec![MSG_CONTAINER as i32, count as i32];
            let mut big = self.next_id();
            for handle in [bind, ping].into_iter().flatten() {
                self.place_to_container(
                    &mut body,
                    &mut big,
                    if bind == Some(handle) { false } else { force },
                    handle,
                );
                let id = self
                    .data
                    .store
                    .lock()
                    .unwrap()
                    .get(handle)
                    .unwrap()
                    .get_msg_id();
                if bind == Some(handle) {
                    self.bind_msg_id = id;
                    self.bind_sent = self.host.now();
                } else {
                    self.ping_msg_id = id;
                }
                tracking.push(id);
                any = true;
                service_handles.push(handle);
            }
            for &(_, handle) in &requests {
                let id = self.prepare_to_send(handle, big, force);
                if id >= big {
                    big = self.next_id();
                }
                let data = self.data.clone();
                let mut queues = data.queues.lock().unwrap();
                let mut store = data.store.lock().unwrap();
                let r = store.get_mut(handle).unwrap();
                let init = needs_layer && r.needs_layer;
                if r.request_id != 0 {
                    if r.need_ack() {
                        r.last_sent_time = self.host.now();
                        queues.have_sent.entry(id).or_insert(handle);
                        tracking.push(id);
                        check = true;
                        any = true;
                    } else {
                        self.acked.entry(id).or_insert(r.request_id);
                    }
                }
                drop(store);
                drop(queues);
                let msg = self.wrap_invoke_after(handle, init);
                append_message(&mut body, &msg);
            }
            for handle in [state, resend, ack, http].into_iter().flatten() {
                self.place_to_container(&mut body, &mut big, force, handle);
                let id = self
                    .data
                    .store
                    .lock()
                    .unwrap()
                    .get(handle)
                    .unwrap()
                    .get_msg_id();
                if state == Some(handle) || resend == Some(handle) {
                    self.state_requests.insert(id, handle);
                    any = true;
                } else {
                    service_handles.push(handle);
                }
            }
            let seq = self.next_request_seq_number(false);
            self.containers.insert(
                big,
                SentContainer {
                    sent: self.host.now(),
                    messages: tracking,
                },
            );
            if check && !self.timers.check.is_active() {
                self.timers.check.call_once(10000);
            }
            Message {
                salt: self.salt,
                session_id: self.session_id,
                msg_id: big,
                seq_no: seq,
                body,
            }
        };
        {
            let mut queues = self.data.queues.lock().unwrap();
            for &(id, _) in &requests {
                queues.to_send.remove(&id);
            }
        }
        self.send_secure_request(outgoing, source, any);
        for handle in service_handles {
            self.data.store.lock().unwrap().remove(handle);
        }
        if send_all && !self.data.queues.lock().unwrap().to_send.is_empty() {
            self.data.queue_need_to_resume_and_send();
        }
    }
    fn send_secure_request(
        &mut self,
        message: Message,
        source: Option<RequestHandle>,
        any: bool,
    ) -> bool {
        let mut request = body_request(message.body.clone(), self.host.now());
        request.set_msg_id(message.msg_id);
        request.set_seq_no(message.seq_no);
        request.add_padding(false, &mut self.random);
        let padding = tg_tl::primes_to_bytes(&request.words[8 + message.body.len()..]);
        let Ok(packet) = encrypt_message(
            self.key.as_ref().unwrap(),
            &message,
            &padding,
            Direction::ClientToServer,
        ) else {
            return false;
        };
        if let Some(handle) = source {
            let mut store = self.data.store.lock().unwrap();
            if let Some(original) = store.get_mut(handle) {
                request.words[0] = self.salt as i32;
                request.words[1] = (self.salt >> 32) as i32;
                request.words[2] = self.session_id as i32;
                request.words[3] = (self.session_id >> 32) as i32;
                original.words = request.words;
            }
        }
        if let Some(connection) = &mut self.connection {
            connection.sent_key = self.key_id;
            let size = packet.len() * 4;
            self.actions
                .push_back(TransportAction::Send(connection.spec.id, packet));
            if any {
                self.on_sent_some(size as u64);
            }
            true
        } else {
            false
        }
    }
    fn send_ping_by_timer(&mut self) {
        if self.ping_id != 0 {
            let till = self.ping_send_at + 15000;
            let now = self.host.now();
            if till < now + 1000 {
                self.restart();
            } else {
                self.timers.ping.call_once(till - now);
            }
        } else {
            self.data.queue_need_to_resume_and_send();
        }
    }
    pub fn send_ping_force(&mut self) {
        if self.ping_id == 0 {
            self.ping_send_at = 0;
            self.try_to_send();
        }
    }
    fn check_sent_requests(&mut self) {
        let now = self.host.now();
        let check = now - 10000;
        if self.bind_msg_id != 0 && self.bind_sent < check {
            self.restart();
            self.timers.check.call_once(10000);
            return;
        }
        let mut requesting = false;
        let mut next = 10000;
        {
            let queues = self.data.queues.lock().unwrap();
            let mut store = self.data.store.lock().unwrap();
            for (&id, &handle) in &queues.have_sent {
                let r = store.get_mut(handle).unwrap();
                if r.last_sent_time <= check {
                    r.last_sent_time = now;
                    requesting |= self.state_ids.insert(id);
                } else {
                    next = next.min(r.last_sent_time - check);
                }
            }
        }
        if requesting {
            self.data.queue_send_anything(1000);
        }
        if next < 10000 {
            self.timers.check.call_once(next);
        }
    }
    fn clear_old_containers(&mut self) {
        let check = self.host.now() - 600000;
        let ids: Vec<_> = self
            .containers
            .iter()
            .filter(|(_, c)| c.sent <= check)
            .map(|(&id, _)| id)
            .collect();
        let mut resent = false;
        for id in ids {
            if let Some(c) = self.containers.remove(&id) {
                resent |= !c.messages.is_empty();
                for id in c.messages {
                    self.resend(id, -1);
                }
            }
        }
        if resent {
            self.data.queue_need_to_resume_and_send();
        }
        let next = self
            .containers
            .values()
            .map(|c| c.sent - check)
            .min()
            .unwrap_or(600000)
            .min(600000);
        if next < 600000 {
            self.timers.clear.call_once(next);
        } else if !self.timers.clear.is_active() {
            self.timers.clear.call_each(next);
        }
    }
    pub fn was_sent(&self, id: u64) -> i32 {
        if id == self.ping_msg_id || id == self.bind_msg_id {
            return -1;
        }
        if let Some(&r) = self.resending.get(&id) {
            return r;
        }
        if let Some(&r) = self.acked.get(&id) {
            return r;
        }
        if self.containers.contains_key(&id) {
            return -1;
        }
        let queues = self.data.queues.lock().unwrap();
        let store = self.data.store.lock().unwrap();
        queues
            .have_sent
            .get(&id)
            .and_then(|h| store.get(*h))
            .map_or(0, |r| if r.request_id != 0 { r.request_id } else { -1 })
    }
    fn clear_special_msg_id(&mut self, id: u64) {
        if id == self.ping_msg_id {
            self.ping_msg_id = 0;
            self.ping_id = 0;
        } else if id == self.bind_msg_id {
            self.bind_msg_id = 0;
        }
    }
    fn resend(&mut self, id: u64, wait: i64) {
        if let Some(c) = self.containers.remove(&id) {
            for id in c.messages {
                self.resend(id, -1);
            }
        } else {
            let mut queues = self.data.queues.lock().unwrap();
            if let Some(h) = queues.have_sent.remove(&id) {
                let mut store = self.data.store.lock().unwrap();
                let r = store.get_mut(h).unwrap();
                r.last_sent_time = self.host.now();
                r.force_send_in_container = true;
                self.resending.entry(id).or_insert(r.request_id);
                queues.to_send.entry(r.request_id).or_insert(h);
            }
        }
        self.clear_special_msg_id(id);
        if wait >= 0 {
            self.data.queue_send_anything(wait);
        }
    }
    fn resend_all(&mut self) {
        let mut queues = self.data.queues.lock().unwrap();
        let mut store = self.data.store.lock().unwrap();
        for (id, h) in std::mem::take(&mut queues.have_sent) {
            let r = store.get_mut(h).unwrap();
            r.last_sent_time = self.host.now();
            r.force_send_in_container = true;
            self.resending.entry(id).or_insert(r.request_id);
            queues.to_send.entry(r.request_id).or_insert(h);
        }
        drop(store);
        drop(queues);
        self.data.queue_send_anything(0);
    }
    fn requests_acked(&mut self, ids: &[u64], by_response: bool) {
        let mut more = vec![];
        let mut queues = self.data.queues.lock().unwrap();
        let mut store = self.data.store.lock().unwrap();
        for &id in ids {
            if let Some(c) = self.containers.remove(&id) {
                more.extend(c.messages);
                continue;
            }
            if let Some(handle) = self.state_requests.remove(&id) {
                store.remove(handle);
                continue;
            }
            if let Some(&handle) = queues.have_sent.get(&id) {
                let request_id = store.get(handle).unwrap().request_id;
                if !by_response && self.data.environment.has_callback(request_id) {
                    continue;
                }
                queues.have_sent.remove(&id);
                self.acked.entry(id).or_insert(request_id);
                continue;
            }
            if let Some(&request_id) = self.resending.get(&id) {
                if !by_response && self.data.environment.has_callback(request_id) {
                    continue;
                }
                self.resending.remove(&id);
                if let Some(handle) = queues.to_send.remove(&request_id) {
                    self.acked
                        .entry(id)
                        .or_insert(store.get(handle).unwrap().request_id);
                }
            }
        }
        drop(store);
        drop(queues);
        while self.acked.len() > 400 {
            self.acked.pop_first();
        }
        if !more.is_empty() {
            self.requests_acked(&more, false);
        }
    }
    fn handle_msgs_states(&mut self, ids: &[u64], states: &[u8]) {
        if ids.len() != states.len() {
            return;
        }
        let mut acked = vec![];
        for (&id, &state) in ids.iter().zip(states) {
            let sent = self.data.queues.lock().unwrap().have_sent.contains_key(&id);
            if !sent {
                if self.resending.contains_key(&id) && state & 7 == 4 {
                    acked.push(id);
                }
            } else if state & 7 != 4 {
                self.resend(id, 10);
            } else {
                acked.push(id);
            }
        }
        self.requests_acked(&acked, false);
    }
    fn correct_unixtime_with_bad_local(&self, time: i32) {
        SYNC_TIME_REQUEST_DURATION.store(500, Ordering::Relaxed);
        self.clock.update_time(time, true);
    }
    fn correct_unixtime_by_fast_request(&self, ids: &[u64], time: i32) {
        let now = self.host.now();
        let queues = self.data.queues.lock().unwrap();
        let store = self.data.store.lock().unwrap();
        for id in ids {
            if let Some(r) = queues.have_sent.get(id).and_then(|h| store.get(*h)) {
                let duration = now - r.last_sent_time;
                if duration >= 0 && duration <= SYNC_TIME_REQUEST_DURATION.load(Ordering::Relaxed) {
                    SYNC_TIME_REQUEST_DURATION.store(duration, Ordering::Relaxed);
                    self.clock.update_time(time, false);
                    return;
                }
            }
        }
    }
    fn requests_fix_time_salt(&mut self, ids: &[u64], info: OuterInfo) -> bool {
        if ids.iter().any(|&id| self.was_sent(id) != 0) {
            if info.server_salt != 0 {
                self.salt = info.server_salt;
            }
            self.correct_unixtime_with_bad_local(info.server_time);
            true
        } else {
            false
        }
    }
    pub fn on_packet(&mut self, id: u64, packet: Buffer, need_http_wait: bool) {
        if self.connection.as_ref().is_none_or(|c| c.spec.id != id) {
            return;
        }
        if self
            .key_creator
            .as_ref()
            .is_some_and(|c| c.creator.is_some())
        {
            self.on_received_some();
            let body = match parse_not_secure_response(&packet) {
                Ok(body) => body,
                Err(_) => {
                    self.restart();
                    return;
                }
            };
            let keys = self
                .data
                .environment
                .dc_options()
                .public_keys(bare_dc_id(self.data.shifted_dc_id));
            let creator = self.key_creator.as_mut().unwrap().creator.as_mut().unwrap();
            let result = creator.receive(body, &keys, self.clock.now(), &mut self.random);
            if let Some(time) = creator.take_sync_time() {
                self.clock.update_time(time, false);
            }
            match result {
                Ok(CreationStep::Send(body)) => self.send_unencrypted(body),
                Ok(CreationStep::Ready(result)) => self.unbound_ready(result),
                Err(error) => self.key_creation_failed(error),
            }
            return;
        }
        self.handle_received(packet, need_http_wait);
    }
    fn handle_received(&mut self, packet: Buffer, need_http_wait: bool) {
        let Some(key) = &self.key else {
            self.restart();
            return;
        };
        let result = decrypt_message(key, &packet, self.session_id, self.clock.now().unix_seconds);
        self.on_received_some();
        let result = match result {
            Ok(result) => result,
            Err(_) => {
                self.restart();
                return;
            }
        };
        let message = result.message;
        let was_connected = self.state == CONNECTED;
        let mut server_salt = message.salt;
        if server_salt != self.salt {
            if !result.bad_time {
                self.salt = server_salt;
                if self.set_state(CONNECTED, Some(CONNECTING)) {
                    self.resend_all();
                }
            }
        } else {
            server_salt = 0;
        }
        if message.needs_ack() {
            self.ack.push(message.msg_id);
        }
        let mut budget = 32 * 1024 * 1024;
        let outcome = match self
            .received
            .register_msg_id(message.msg_id, message.needs_ack())
        {
            IdResult::Success => self.handle_one_received(
                &message.body,
                message.msg_id,
                OuterInfo {
                    outer_msg_id: message.msg_id,
                    server_salt,
                    server_time: (message.msg_id >> 32) as i32,
                    bad_time: result.bad_time,
                },
                &mut budget,
                0,
            ),
            IdResult::TooOld => HandleResult::ResetSession,
            IdResult::Duplicate => HandleResult::Success,
        };
        self.received.shrink();
        if !self.ack.is_empty() {
            self.data.queue_send_anything(10000);
        }
        if !self.data.queues.lock().unwrap().received.is_empty() {
            self.data.queue_try_to_receive();
        }
        match outcome {
            HandleResult::Success | HandleResult::Ignored => {}
            HandleResult::DestroyTemporaryKey => {
                self.destroy_temporary_key();
                self.restart();
                return;
            }
            HandleResult::ResetSession => {
                self.need_session_reset = true;
                self.restart();
                return;
            }
            _ => {
                self.restart();
                return;
            }
        }
        self.retry_timeout = 1;
        self.started_connecting_at = 0;
        if !was_connected && self.state == CONNECTED {
            self.data.queue_need_to_resume_and_send();
        }
        if need_http_wait {
            self.data.queue_send_anything(0);
        }
    }
    fn queue_response(&self, reply: Buffer, request_id: i32, outer_msg_id: u64) {
        self.data
            .queues
            .lock()
            .unwrap()
            .received
            .push_back(Response {
                reply,
                request_id,
                outer_msg_id,
            });
    }
    fn ungzip(&self, words: &[i32], limit: usize) -> Option<Buffer> {
        if limit == 0 {
            return None;
        }
        let packed = tg_tl::read_string(&mut tg_tl::Reader::new(words)).ok()?;
        let data = tg_base::zlib_help::inflate_gzip(&packed, limit).ok()?;
        if data.is_empty() || !data.len().is_multiple_of(4) {
            None
        } else {
            Some(tg_tl::bytes_to_primes(&data))
        }
    }
    fn handle_one_received(
        &mut self,
        body: &[i32],
        _msg_id: u64,
        mut info: OuterInfo,
        budget: &mut usize,
        depth: usize,
    ) -> HandleResult {
        use HandleResult::*;
        tg_base::expects!(!body.is_empty());
        let constructor = body[0];
        let mut cursor = Cursor::new(&body[1..]);
        match constructor as u32 {
            GZIP_PACKED => {
                if depth >= 64 {
                    return RestartConnection;
                }
                let Some(unpacked) = self.ungzip(&body[1..], *budget) else {
                    return RestartConnection;
                };
                *budget -= unpacked.len() * 4;
                self.handle_one_received(&unpacked, _msg_id, info, budget, depth + 1)
            }
            MSG_CONTAINER => {
                let Some(count) = cursor.int().map(|n| n as u32) else {
                    return ParseError;
                };
                for _ in 0..count {
                    let Some(id) = cursor.long() else {
                        return ParseError;
                    };
                    if !matches!(id & 3, 1 | 3) {
                        return RestartConnection;
                    }
                    let Some(seq) = cursor.int() else {
                        return ParseError;
                    };
                    let Some(length) = cursor.int() else {
                        return ParseError;
                    };
                    if length < 4 || length & 3 != 0 {
                        return RestartConnection;
                    }
                    if seq & 1 != 0 {
                        self.ack.push(id);
                    }
                    let Some(nested) = cursor.take(length as usize / 4) else {
                        return ParseError;
                    };
                    let result = match self.received.register_msg_id(id, seq & 1 != 0) {
                        IdResult::Success => {
                            let result = self.handle_one_received(nested, id, info, budget, depth);
                            info.bad_time = false;
                            result
                        }
                        IdResult::TooOld => ResetSession,
                        IdResult::Duplicate => Success,
                    };
                    if result != Success {
                        return result;
                    }
                }
                Success
            }
            0x62d6b459 => {
                let Some(ids) = cursor.ids() else {
                    return ParseError;
                };
                if ids.is_empty() {
                    return if info.bad_time { Ignored } else { Success };
                }
                if info.bad_time {
                    if !self.requests_fix_time_salt(&ids, info) {
                        return Ignored;
                    }
                } else {
                    self.correct_unixtime_by_fast_request(&ids, info.server_time);
                }
                self.requests_acked(&ids, false);
                Success
            }
            0xa7eff811 => {
                let Some(id) = cursor.long() else {
                    return ParseError;
                };
                if cursor.int().is_none() {
                    return ParseError;
                }
                let Some(code) = cursor.int() else {
                    return ParseError;
                };
                if matches!(code, 16 | 17 | 32 | 33 | 64) {
                    if self.was_sent(id) == 0 {
                        return if info.bad_time { Ignored } else { Success };
                    }
                    if matches!(code, 16 | 17 | 64) {
                        if info.server_salt != 0 {
                            self.salt = info.server_salt;
                        }
                        self.correct_unixtime_with_bad_local(info.server_time);
                        self.resend(id, 0);
                        Success
                    } else {
                        if info.bad_time {
                            if info.server_salt != 0 {
                                self.salt = info.server_salt;
                            }
                            self.correct_unixtime_with_bad_local(info.server_time);
                        }
                        if self.bind_msg_id != 0 {
                            RestartConnection
                        } else {
                            ResetSession
                        }
                    }
                } else {
                    let request_id = self.was_sent(id);
                    if self.bind_msg_id != 0 {
                        return RestartConnection;
                    }
                    if request_id != 0 {
                        let error = tg_scheme::mtp_rpc_error(500, b"PROTOCOL_ERROR".to_vec());
                        self.queue_response(
                            tg_tl::serialize(&error),
                            request_id,
                            info.outer_msg_id,
                        );
                    }
                    if info.bad_time {
                        Ignored
                    } else {
                        Success
                    }
                }
            }
            0xedab447b => {
                let Some(id) = cursor.long() else {
                    return ParseError;
                };
                if cursor.int().is_none() || cursor.int().is_none() {
                    return ParseError;
                }
                let Some(salt) = cursor.long() else {
                    return ParseError;
                };
                if self.was_sent(id) == 0 {
                    return if info.bad_time { Ignored } else { Success };
                }
                self.salt = salt;
                self.clock.update_time(info.server_time, false);
                if self.bind_msg_id != 0 {
                    return RestartConnection;
                }
                if self.set_state(CONNECTED, Some(CONNECTING)) {
                    self.resend_all();
                }
                self.resend(id, 0);
                Success
            }
            0x04deb57d => {
                let Some(id) = cursor.long() else {
                    return ParseError;
                };
                let Some(states) = cursor.bytes() else {
                    return ParseError;
                };
                let Some(&handle) = self.state_requests.get(&id) else {
                    return if info.bad_time { Ignored } else { Success };
                };
                if info.bad_time {
                    if info.server_salt != 0 {
                        self.salt = info.server_salt;
                    }
                    self.correct_unixtime_with_bad_local(info.server_time);
                }
                let request_body = {
                    let store = self.data.store.lock().unwrap();
                    store.get(handle).unwrap().body().to_vec()
                };
                self.requests_acked(&[id], true);
                let mut original = Cursor::new(&request_body);
                if !matches!(
                    original.int().map(|n| n as u32),
                    Some(0xda69fb52 | 0x7d861a08)
                ) {
                    return ParseError;
                }
                let Some(ids) = original.ids() else {
                    return ParseError;
                };
                self.handle_msgs_states(&ids, &states);
                Success
            }
            0x8cc0d131 => {
                if info.bad_time {
                    return Ignored;
                }
                let Some(ids) = cursor.ids() else {
                    return ParseError;
                };
                let Some(states) = cursor.bytes() else {
                    return ParseError;
                };
                self.handle_msgs_states(&ids, &states);
                Success
            }
            0x276d3ec6 | 0x809db6df => {
                let old = constructor as u32 == 0x276d3ec6;
                if !old && info.bad_time {
                    return Ignored;
                }
                if old {
                    let Some(id) = cursor.long() else {
                        return ParseError;
                    };
                    if info.bad_time && !self.requests_fix_time_salt(&[id], info) {
                        return Ignored;
                    }
                    self.requests_acked(&[id], false);
                }
                let Some(answer) = cursor.long() else {
                    return ParseError;
                };
                if cursor.int().is_none() || cursor.int().is_none() {
                    return ParseError;
                }
                if self.received.lookup(answer) != IdState::NotFound {
                    self.ack.push(answer);
                } else {
                    self.resend_ids.push(answer);
                }
                Success
            }
            RPC_RESULT => {
                let Some(id) = cursor.long() else {
                    return ParseError;
                };
                if cursor.remaining().is_empty() {
                    return ParseError;
                }
                if info.bad_time && !self.requests_fix_time_salt(&[id], info) {
                    return Ignored;
                }
                let reply = if cursor.remaining()[0] as u32 == GZIP_PACKED {
                    if depth >= 64 {
                        return RestartConnection;
                    }
                    let Some(unpacked) = self.ungzip(&cursor.remaining()[1..], *budget) else {
                        return RestartConnection;
                    };
                    *budget -= unpacked.len() * 4;
                    unpacked
                } else {
                    cursor.remaining().to_vec()
                };
                if reply[0] as u32 == 0x2144ca19 {
                    if is_destroyed_temporary_key_error(&reply) {
                        return DestroyTemporaryKey;
                    }
                } else {
                    self.data.notify_connection_inited(&self.options);
                }
                self.requests_acked(&[id], true);
                let bind = self.handle_bind_response(id, &reply);
                if bind != Ignored {
                    return bind;
                }
                let request_id = self.was_sent(id);
                if request_id != 0 && request_id != -1 {
                    self.queue_response(reply, request_id, info.outer_msg_id);
                }
                Success
            }
            0x9ec20908 => {
                let Some(first) = cursor.long() else {
                    return ParseError;
                };
                if cursor.long().is_none() {
                    return ParseError;
                }
                let Some(salt) = cursor.long() else {
                    return ParseError;
                };
                if info.bad_time && !self.requests_fix_time_salt(&[first], info) {
                    return Ignored;
                }
                self.salt = salt;
                let resend: Vec<_> = {
                    let queues = self.data.queues.lock().unwrap();
                    let store = self.data.store.lock().unwrap();
                    queues
                        .have_sent
                        .range(..first)
                        .filter(|(_, h)| store.get(**h).is_some_and(|r| r.request_id != 0))
                        .map(|(&id, _)| id)
                        .collect()
                };
                for id in resend {
                    self.resend(id, 10);
                }
                self.queue_response(body[..7].to_vec(), 0, info.outer_msg_id);
                Success
            }
            0x347773c5 => {
                let Some(id) = cursor.long() else {
                    return ParseError;
                };
                let Some(ping) = cursor.long() else {
                    return ParseError;
                };
                if self.was_sent(id) == 0 {
                    return Ignored;
                }
                if ping == self.ping_id {
                    self.ping_id = 0;
                }
                if info.bad_time && !self.requests_fix_time_salt(&[id], info) {
                    return Ignored;
                }
                self.requests_acked(&[id], true);
                Success
            }
            _ => {
                if info.bad_time {
                    return ResetSession;
                }
                if self.current_dc_type == DcType::Regular {
                    self.queue_response(body.to_vec(), 0, info.outer_msg_id);
                }
                Success
            }
        }
    }
    fn handle_bind_response(&mut self, id: u64, body: &[i32]) -> HandleResult {
        use HandleResult::*;
        if self.key_creator.is_none() || self.bind_msg_id == 0 || self.bind_msg_id != id {
            return Ignored;
        }
        self.bind_msg_id = 0;
        match self
            .key_creator
            .as_ref()
            .unwrap()
            .handle_bind_response(body)
        {
            DcKeyBindState::Success => {
                let persistent = self.key_creator.take().unwrap().bind_persistent_key();
                if !self
                    .data
                    .release_key_creation_on_done(self.key.as_ref().unwrap().clone(), persistent)
                {
                    return DestroyTemporaryKey;
                }
                self.data.queue_need_to_resume_and_send();
                Success
            }
            DcKeyBindState::DefinitelyDestroyed => {
                if self.destroy_old_enough_persistent_key() {
                    DestroyTemporaryKey
                } else {
                    self.data.queue_need_to_resume_and_send();
                    Success
                }
            }
            DcKeyBindState::Failed => {
                self.data.queue_need_to_resume_and_send();
                Success
            }
        }
    }
}
impl<C: SessionClock, R: Random> Drop for SessionPrivate<C, R> {
    fn drop(&mut self) {
        self.release_key_creation_on_fail();
        self.do_disconnect();
        self.data.snapshot.lock().unwrap().private_alive = false;
        let mut store = self.data.store.lock().unwrap();
        for (_, handle) in std::mem::take(&mut self.state_requests) {
            store.remove(handle);
        }
    }
}
fn body_request(body: Buffer, now: i64) -> SerializedRequest {
    assert!(!body.is_empty());
    let mut request = SerializedRequest::prepare(body.len(), 0, now);
    request.words.extend(body);
    request
}
fn id_vector(constructor: u32, ids: &[u64]) -> Buffer {
    let mut body = vec![constructor as i32, 0x1cb5c415, ids.len() as i32];
    for &id in ids {
        body.extend([id as i32, (id >> 32) as i32]);
    }
    body
}
fn append_message(to: &mut Buffer, message: &Message) {
    to.extend([
        message.msg_id as i32,
        (message.msg_id >> 32) as i32,
        message.seq_no as i32,
        (message.body.len() * 4) as i32,
    ]);
    to.extend_from_slice(&message.body);
}
fn is_media_cluster(id: i32) -> bool {
    let shift = get_dc_id_shift(id);
    (BASE_DOWNLOAD_DC_SHIFT..BASE_DOWNLOAD_DC_SHIFT + MAX_MEDIA_DC_COUNT).contains(&shift)
        || matches!(
            shift,
            GROUP_CALL_STREAM_DC_SHIFT | EXPORT_MEDIA_DC_SHIFT | UPDATER_DC_SHIFT
        )
}
struct Cursor<'a> {
    words: &'a [i32],
    position: usize,
}
impl<'a> Cursor<'a> {
    fn new(words: &'a [i32]) -> Self {
        Self { words, position: 0 }
    }
    fn take(&mut self, count: usize) -> Option<&'a [i32]> {
        let end = self.position.checked_add(count)?;
        let result = self.words.get(self.position..end)?;
        self.position = end;
        Some(result)
    }
    fn int(&mut self) -> Option<i32> {
        Some(self.take(1)?[0])
    }
    fn long(&mut self) -> Option<u64> {
        let words = self.take(2)?;
        Some(words[0] as u32 as u64 | ((words[1] as u32 as u64) << 32))
    }
    fn ids(&mut self) -> Option<Vec<u64>> {
        if self.int()? as u32 != 0x1cb5c415 {
            return None;
        }
        let count = self.int()?;
        if count < 0 || count as usize > self.remaining().len() / 2 {
            return None;
        }
        (0..count).map(|_| self.long()).collect()
    }
    fn bytes(&mut self) -> Option<Vec<u8>> {
        let mut reader = tg_tl::Reader::new(self.remaining());
        let data = tg_tl::read_string(&mut reader).ok()?;
        Some(data)
    }
    fn remaining(&self) -> &'a [i32] {
        &self.words[self.position..]
    }
}

pub fn log_ids_vector(ids: &[u64]) -> String {
    if ids.is_empty() {
        return "[]".into();
    }
    let mut result = format!("[{}", ids[0]);
    for id in ids {
        result.push_str(&format!(", {id}"));
    }
    result.push(']');
    result
}
#[derive(Clone, Copy)]
pub enum AppDistribution {
    Plain,
    MacStore,
    WindowsStore,
    Flatpak,
    Snap,
}
pub fn compute_app_version(
    version: &str,
    architecture: &str,
    windows: bool,
    distribution: AppDistribution,
) -> String {
    let suffix = if windows && architecture == "x86_64" {
        " x64".into()
    } else if architecture == "x86_64" || windows && architecture == "i386" {
        String::new()
    } else {
        format!(" {architecture}")
    };
    let package = match distribution {
        AppDistribution::Plain => "",
        AppDistribution::MacStore => " Mac App Store",
        AppDistribution::WindowsStore => " Microsoft Store",
        AppDistribution::Flatpak => " Flatpak",
        AppDistribution::Snap => " Snap",
    };
    format!("{version}{suffix}{package}")
}

pub const OPTION_PREFER_IPV6: &str = "prefer-ipv6";
pub fn register_options(
    registry: &tg_base::options::Registry,
) -> tg_base::options::OptionHandle<bool> {
    let mut descriptor = tg_base::options::Descriptor::new(OPTION_PREFER_IPV6);
    descriptor.name = "Prefer IPv6".into();
    descriptor.description =
        r#"Prefer IPv6 if it is available. Require "Try connecting through IPv6" to be enabled"#
            .into();
    registry.register(descriptor)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
