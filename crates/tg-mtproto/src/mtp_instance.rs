//! Request ownership, DC routing and main-thread callback dispatch.
pub mod network;
pub mod runtime;
use crate::{
    core_types::*,
    details::{
        mtproto_dcenter::Dcenter,
        mtproto_serialized_request::{RequestHandle, RequestStore, SerializedRequest},
    },
    mtproto_auth_key::AuthKey,
    mtproto_config::Config,
    mtproto_dc_options::{DcOptions, Environment},
    mtproto_response::{Error, Response, ResponseHandler},
    session::{
        EnvironmentChange, InstanceEvent, ProxySettings, SessionCommand, SessionEnvironment,
        SessionHandle,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc, Mutex,
    },
};
use tg_tl::Function;
use tokio::sync::mpsc;
static NEXT_REQUEST: AtomicI32 = AtomicI32::new(0);
pub fn get_next_request_id() -> i32 {
    let result = NEXT_REQUEST.fetch_add(1, Ordering::Relaxed) + 1;
    if result == i32::MAX / 2 {
        NEXT_REQUEST.store(0, Ordering::Relaxed);
    }
    result
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    KeysDestroyer,
}
pub struct Fields {
    pub config: Config,
    pub main_dc_id: i32,
    pub keys: Vec<Arc<AuthKey>>,
    pub device_model: String,
    pub system_version: String,
}
impl Fields {
    pub fn new(environment: Environment) -> Self {
        Self {
            config: Config::new(environment),
            main_dc_id: 0,
            keys: vec![],
            device_model: String::new(),
            system_version: String::new(),
        }
    }
}
pub struct SpawnSession {
    pub id: i32,
    pub dc: Arc<Dcenter>,
}
pub type UpdateHandler = Box<dyn FnMut(&Response) + Send>;
pub type GlobalFailHandler = Box<dyn FnMut(&Error, &Response) + Send>;
pub type StateHandler = Box<dyn FnMut(i32, i32) + Send>;
pub type ResetHandler = Box<dyn FnMut(i32) + Send>;
struct State {
    config: Config,
    main_dc: i32,
    forced: bool,
    mode: Mode,
    device: String,
    system: String,
    languages: [String; 3],
    proxy: ProxySettings,
    dcs: BTreeMap<i32, Arc<Dcenter>>,
    sessions: BTreeMap<i32, SessionHandle>,
    spawns: VecDeque<SpawnSession>,
    keys: BTreeMap<i32, Arc<AuthKey>>,
    requests: BTreeMap<i32, RequestHandle>,
    retired: BTreeSet<RequestHandle>,
    routes: BTreeMap<i32, i32>,
    pending_waits: BTreeMap<i32, i64>,
    callbacks: BTreeMap<i32, ResponseHandler>,
    events: Arc<Mutex<VecDeque<InstanceEvent>>>,
    delayed: VecDeque<(i32, i64)>,
    delays: BTreeMap<i32, i32>,
    dependent: BTreeMap<i32, i32>,
    bad_guest: BTreeSet<i32>,
    waiters: BTreeMap<i32, Vec<i32>>,
    exports: BTreeMap<i32, i32>,
    handler_epochs: [u64; 4],
    resume: mpsc::UnboundedReceiver<()>,
    updates: Option<UpdateHandler>,
    global_fail: Option<GlobalFailHandler>,
    state_changed: Option<StateHandler>,
    reset: Option<ResetHandler>,
    pub_events: VecDeque<Notice>,
    notice_subscribers: Vec<mpsc::UnboundedSender<Notice>>,
    now: i64,
    last_config: i64,
    config_expires: i64,
    config_loading: bool,
    phone: String,
    resolver: crate::details::mtproto_domain_resolver::DomainResolver,
    resolve_work: VecDeque<crate::details::mtproto_domain_resolver::Request>,
    cdn_request: i32,
    http_time_loading: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    WriteKeys,
    TemporaryKeyChanged(i32),
    AllKeysDestroyed,
    MainDc(i32),
    Restarted(i32),
    NonPremiumDelayed(i32),
    Frozen,
    LanguageReset,
    BadConfiguration,
    ConfigNeeded,
    ConfigApplied {
        suggested_lang: String,
        lang_version: i32,
        base_lang_version: i32,
        autoupdate_prefix: Option<String>,
    },
    CdnConfigNeeded,
    HttpTimeNeeded,
}
#[derive(Clone)]
pub struct Instance {
    environment: Arc<EnvironmentBridge>,
    state: Arc<Mutex<State>>,
    pub store: Arc<Mutex<RequestStore>>,
}
impl Instance {
    pub fn new(mode: Mode, fields: Fields) -> Self {
        let environment = Arc::new(EnvironmentBridge::new(&fields, mode));
        let mut keys = BTreeMap::new();
        let mut dcs = BTreeMap::new();
        for key in fields.keys {
            let mut id = key.dc_id();
            if mode == Mode::KeysDestroyer {
                id = crate::facade::destroy_key_next_dc_id(id);
                while keys.contains_key(&id) {
                    id = crate::facade::destroy_key_next_dc_id(id);
                }
            }
            dcs.insert(
                id,
                Arc::new(Dcenter::new(bare_dc_id(id), Some(key.clone()))),
            );
            keys.insert(id, key);
        }
        let main = if fields.main_dc_id == 0 {
            2
        } else {
            fields.main_dc_id
        };
        let result = Self {
            environment: environment.clone(),
            store: Arc::new(Mutex::new(RequestStore::default())),
            state: Arc::new(Mutex::new(State {
                config: fields.config,
                main_dc: main,
                forced: fields.main_dc_id != 0,
                mode,
                device: fields.device_model,
                system: fields.system_version,
                languages: ["en".into(), "en".into(), "tdesktop".into()],
                proxy: ProxySettings::default(),
                dcs,
                sessions: BTreeMap::new(),
                spawns: VecDeque::new(),
                keys,
                requests: BTreeMap::new(),
                retired: BTreeSet::new(),
                routes: BTreeMap::new(),
                pending_waits: BTreeMap::new(),
                callbacks: BTreeMap::new(),
                events: Arc::new(Mutex::new(VecDeque::new())),
                delayed: VecDeque::new(),
                delays: BTreeMap::new(),
                dependent: BTreeMap::new(),
                bad_guest: BTreeSet::new(),
                waiters: BTreeMap::new(),
                exports: BTreeMap::new(),
                handler_epochs: [0; 4],
                resume: crate::facade::unpaused(),
                updates: None,
                global_fail: None,
                state_changed: None,
                reset: None,
                pub_events: VecDeque::new(),
                notice_subscribers: vec![],
                now: 0,
                last_config: 0,
                config_expires: 0,
                config_loading: false,
                phone: String::new(),
                resolver: Default::default(),
                resolve_work: VecDeque::new(),
                cdn_request: 0,
                http_time_loading: false,
            })),
        };
        result.state.lock().unwrap().events = environment.events.clone();
        result.sync_environment();
        result
    }
    pub fn start(&self) {
        let ids = {
            let s = self.state.lock().unwrap();
            if s.mode == Mode::KeysDestroyer {
                s.dcs.keys().copied().collect()
            } else {
                vec![s.main_dc]
            }
        };
        for id in ids {
            self.ensure_session(id);
            if self.is_keys_destroyer() {
                self.schedule_key_destroy(id);
            }
        }
        self.request_config();
    }
    pub fn main_dc_id(&self) -> i32 {
        self.state.lock().unwrap().main_dc
    }
    pub fn suggest_main_dc_id(&self, id: i32) {
        if !self.state.lock().unwrap().forced {
            self.set_main_dc_id(id);
        }
    }
    pub fn set_main_dc_id(&self, id: i32) {
        let old = self.main_dc_id();
        if old != id {
            self.schedule_session_destroy(old);
            self.schedule_session_destroy(id);
            self.state.lock().unwrap().main_dc = id;
            self.ensure_session(id);
        }
        let mut s = self.state.lock().unwrap();
        s.forced = true;
        s.notice(Notice::MainDc(id));
        s.notice(Notice::WriteKeys);
    }
    pub fn resolved_dc_id(&self, id: i32) -> i32 {
        let main = self.main_dc_id();
        if id == 0 {
            main
        } else if bare_dc_id(id) == 0 {
            id + main
        } else {
            id
        }
    }
    pub fn ensure_session(&self, id: i32) -> i32 {
        let id = self.resolved_dc_id(id);
        let mut s = self.state.lock().unwrap();
        if !s.sessions.contains_key(&id) && !s.spawns.iter().any(|x| x.id == id) {
            let real = crate::facade::get_real_id_from_temporary_dc_id(id);
            let dc_id = if s.mode == Mode::KeysDestroyer {
                id
            } else if real != 0 {
                real
            } else {
                bare_dc_id(id)
            };
            let dc = s
                .dcs
                .entry(dc_id)
                .or_insert_with(|| Arc::new(Dcenter::new(bare_dc_id(dc_id), None)))
                .clone();
            s.spawns.push_back(SpawnSession { id, dc });
        }
        id
    }
    pub fn take_session_spawns(&self) -> VecDeque<SpawnSession> {
        std::mem::take(&mut self.state.lock().unwrap().spawns)
    }
    pub fn attach_session(&self, id: i32, handle: SessionHandle) {
        self.state.lock().unwrap().sessions.insert(id, handle);
        let pending: Vec<_> = {
            let s = self.state.lock().unwrap();
            s.routes
                .iter()
                .filter(|(_, dc)| dc.abs() == id)
                .filter(|(rid, _)| !s.dependent.contains_key(rid))
                .filter_map(|(rid, _)| {
                    s.requests.get(rid).copied().map(|h| {
                        (
                            h,
                            (s.pending_waits.get(rid).copied().unwrap_or(s.now) - s.now).max(0),
                        )
                    })
                })
                .collect()
        };
        for (handle, wait) in pending {
            self.send_handle(id, handle, wait);
        }
    }
    pub fn session(&self, id: i32) -> Option<SessionHandle> {
        let id = self.resolved_dc_id(id);
        self.state.lock().unwrap().sessions.get(&id).cloned()
    }
    fn command(&self, id: i32, command: SessionCommand) {
        if let Some(h) = self.session(id) {
            h.send(command);
        }
    }
    fn send_handle(&self, id: i32, handle: RequestHandle, wait: i64) {
        self.command(
            id,
            SessionCommand::Send {
                handle,
                can_wait: wait,
            },
        );
    }
    pub fn send<R: Function>(
        &self,
        request: &R,
        callbacks: ResponseHandler,
        dc: i32,
        wait: i64,
        after: i32,
        override_id: i32,
    ) -> i32 {
        let id = if override_id == 0 {
            get_next_request_id()
        } else {
            override_id
        };
        let now = self.state.lock().unwrap().now;
        self.send_serialized(
            id,
            SerializedRequest::serialize(request, now),
            callbacks,
            dc,
            wait,
            after,
            true,
        );
        id
    }
    #[allow(clippy::too_many_arguments)]
    pub fn send_serialized(
        &self,
        id: i32,
        mut request: SerializedRequest,
        callbacks: ResponseHandler,
        dc: i32,
        wait: i64,
        after: i32,
        needs_layer: bool,
    ) {
        let target = self.ensure_session(dc);
        request.request_id = id;
        request.needs_layer = needs_layer;
        let (handle, blocked) = {
            let mut s = self.state.lock().unwrap();
            request.last_sent_time = s.now;
            request.after = s.requests.get(&after).copied();
            let blocked = request.after.is_some() && s.dependent.contains_key(&after);
            if blocked {
                s.dependent.insert(id, after);
            }
            let handle = self.store.lock().unwrap().insert(request);
            s.requests.insert(id, handle);
            let send_at = s.now + wait;
            s.pending_waits.insert(id, send_at);
            s.routes.insert(id, if dc == 0 { -target } else { target });
            if callbacks.done.is_some() || callbacks.fail.is_some() {
                s.callbacks.insert(id, callbacks);
            }
            (handle, blocked)
        };
        self.sync_environment();
        if !blocked {
            self.send_handle(target, handle, wait);
        }
    }
    pub fn cancel(&self, id: i32) {
        if id == 0 {
            return;
        }
        let (route, handle) = {
            let mut s = self.state.lock().unwrap();
            s.callbacks.remove(&id);
            (s.routes.get(&id).copied(), s.requests.get(&id).copied())
        };
        let msg_id = handle
            .and_then(|h| self.store.lock().unwrap().get(h).map(|r| r.get_msg_id()))
            .unwrap_or(0);
        if let Some(dc) = route {
            self.command(
                dc.abs(),
                SessionCommand::Cancel {
                    request_id: id,
                    msg_id,
                },
            );
        }
        self.unregister_request(id);
    }
    pub fn unregister_request(&self, id: i32) {
        let (removed, resend) = {
            let mut s = self.state.lock().unwrap();
            s.delays.remove(&id);
            s.pending_waits.remove(&id);
            s.routes.remove(&id);
            let removed = s.requests.remove(&id);
            let mut affected = BTreeSet::from([id]);
            loop {
                let before = affected.len();
                for (child, parent) in &s.dependent {
                    if affected.contains(parent) {
                        affected.insert(*child);
                    }
                }
                if before == affected.len() {
                    break;
                }
            }
            let resend: Vec<_> = affected.iter().filter(|x| **x != id).copied().collect();
            s.dependent.retain(|child, _| !affected.contains(child));
            (removed, resend)
        };
        if let Some(h) = removed {
            self.state.lock().unwrap().retired.insert(h);
        }
        self.collect_requests();
        self.sync_environment();
        for rid in resend {
            self.resend(rid);
        }
    }
    fn resend(&self, id: i32) {
        let route = {
            let s = self.state.lock().unwrap();
            s.routes
                .get(&id)
                .zip(s.requests.get(&id))
                .map(|(dc, h)| (dc.abs(), *h))
        };
        if let Some((dc, h)) = route {
            self.ensure_session(dc);
            self.send_handle(dc, h, 0);
        }
    }
    pub fn state(&self, id: i32) -> i32 {
        if id <= 0 {
            return self.session(-id).map_or(0, |s| s.request_state(0));
        }
        let route = self.state.lock().unwrap().routes.get(&id).copied();
        route
            .and_then(|dc| self.session(dc.abs()))
            .map_or(0, |s| s.request_state(id))
    }
    pub fn dcstate(&self, id: i32) -> i32 {
        self.session(id).map_or(0, |s| s.get_state())
    }
    pub fn dctransport(&self, id: i32) -> String {
        self.session(id).map_or_else(String::new, |s| s.transport())
    }
    pub fn ping(&self) {
        self.ensure_session(0);
        self.command(0, SessionCommand::Ping);
    }
    pub fn restart(&self, dc: Option<i32>) {
        let handles: Vec<_> = self
            .state
            .lock()
            .unwrap()
            .sessions
            .iter()
            .filter(|(id, _)| dc.is_none_or(|dc| bare_dc_id(**id) == bare_dc_id(dc)))
            .map(|(_, h)| h.clone())
            .collect();
        for h in handles {
            h.send(SessionCommand::Restart);
        }
    }
    pub fn re_init_connection(&self, dc: i32) {
        let handles: Vec<_> = self
            .state
            .lock()
            .unwrap()
            .sessions
            .iter()
            .filter(|(id, _)| bare_dc_id(**id) == dc)
            .map(|(_, h)| h.clone())
            .collect();
        for h in handles {
            h.send(SessionCommand::ReInitConnection);
        }
    }
    fn schedule_session_destroy(&self, id: i32) {
        let h = {
            let mut s = self.state.lock().unwrap();
            s.spawns.retain(|x| x.id != id);
            s.sessions.remove(&id)
        };
        if let Some(h) = h {
            h.send(SessionCommand::Kill);
        }
    }
    pub fn kill_session(&self, id: i32) {
        assert_ne!(id, self.main_dc_id());
        self.schedule_session_destroy(id);
    }
    pub fn stop_session(&self, id: i32) {
        if id != self.main_dc_id() {
            self.command(id, SessionCommand::Stop);
        }
    }
    pub fn get_keys_for_write(&self) -> Vec<Arc<AuthKey>> {
        self.state.lock().unwrap().keys.values().cloned().collect()
    }
    pub fn take_notices(&self) -> VecDeque<Notice> {
        std::mem::take(&mut self.state.lock().unwrap().pub_events)
    }
    pub fn request_config(&self) {
        let mut s = self.state.lock().unwrap();
        if s.mode != Mode::KeysDestroyer && !s.config_loading {
            s.config_loading = true;
            s.notice(Notice::ConfigNeeded);
        }
    }
    pub fn process(&self, now: i64) {
        assert!(tg_runtime::main_queue().is_main_thread());
        self.state.lock().unwrap().now = now;
        let resume = {
            let mut s = self.state.lock().unwrap();
            let mut changed = false;
            while s.resume.try_recv().is_ok() {
                changed = true;
            }
            changed
        };
        if resume {
            self.unpaused();
        }
        loop {
            let event = self.environment.events.lock().unwrap().pop_front();
            let Some(event) = event else {
                break;
            };
            self.process_event(event);
        }
    }
    pub fn check_delayed_requests(&self, now: i64) {
        loop {
            let due = {
                let mut s = self.state.lock().unwrap();
                if s.delayed.front().is_some_and(|(_, at)| *at <= now) {
                    s.delayed.pop_front().map(|x| x.0)
                } else {
                    None
                }
            };
            match due {
                Some(id) => self.resend(id),
                None => break,
            }
        }
    }
    pub fn next_delayed_deadline(&self) -> Option<i64> {
        self.state.lock().unwrap().delayed.front().map(|x| x.1)
    }
    pub fn set_updates_handler(&self, h: Option<UpdateHandler>) {
        let mut s = self.state.lock().unwrap();
        s.handler_epochs[0] += 1;
        s.updates = h;
    }
    pub fn set_global_fail_handler(&self, h: Option<GlobalFailHandler>) {
        let mut s = self.state.lock().unwrap();
        s.handler_epochs[1] += 1;
        s.global_fail = h;
    }
    pub fn set_state_changed_handler(&self, h: Option<StateHandler>) {
        let mut s = self.state.lock().unwrap();
        s.handler_epochs[2] += 1;
        s.state_changed = h;
    }
    pub fn set_session_reset_handler(&self, h: Option<ResetHandler>) {
        let mut s = self.state.lock().unwrap();
        s.handler_epochs[3] += 1;
        s.reset = h;
    }
    pub fn clear_global_handlers(&self) {
        let mut s = self.state.lock().unwrap();
        for epoch in &mut s.handler_epochs {
            *epoch += 1;
        }
        s.updates = None;
        s.global_fail = None;
        s.state_changed = None;
        s.reset = None;
    }
    fn broadcast(&self, change: EnvironmentChange) {
        self.environment
            .changes
            .lock()
            .unwrap()
            .retain(|tx| tx.send(change.clone()).is_ok());
    }
    fn process_event(&self, event: InstanceEvent) {
        match event {
            InstanceEvent::Callback(r) => self.process_callback(&r),
            InstanceEvent::Update(r) => {
                let (mut h, epoch) = {
                    let mut s = self.state.lock().unwrap();
                    (s.updates.take(), s.handler_epochs[0])
                };
                if let Some(f) = &mut h {
                    f(&r);
                }
                let mut s = self.state.lock().unwrap();
                if s.handler_epochs[0] == epoch {
                    s.updates = h;
                }
            }
            InstanceEvent::StateChange(dc, state) => {
                let (mut h, epoch) = {
                    let mut s = self.state.lock().unwrap();
                    (s.state_changed.take(), s.handler_epochs[2])
                };
                if let Some(f) = &mut h {
                    f(dc, state);
                }
                let mut s = self.state.lock().unwrap();
                if s.handler_epochs[2] == epoch {
                    s.state_changed = h;
                }
            }
            InstanceEvent::SessionReset(dc) => {
                let (mut h, epoch) = {
                    let mut s = self.state.lock().unwrap();
                    (s.reset.take(), s.handler_epochs[3])
                };
                if let Some(f) = &mut h {
                    f(dc);
                }
                let mut s = self.state.lock().unwrap();
                if s.handler_epochs[3] == epoch {
                    s.reset = h;
                }
            }
            InstanceEvent::DcPersistentKeyChanged(dc, key) => {
                self.dc_persistent_key_changed(dc, Some(key))
            }
            InstanceEvent::DcTemporaryKeyChanged(dc) => self.dc_temporary_key_changed(dc),
            InstanceEvent::KeyWasPossiblyDestroyed(dc) => {
                let mut s = self.state.lock().unwrap();
                s.dcs.remove(&dc);
                s.keys.remove(&dc);
                if s.dcs.is_empty() {
                    s.notice(Notice::AllKeysDestroyed);
                }
                drop(s);
                self.schedule_session_destroy(dc);
            }
            InstanceEvent::KeyDestroyedOnServer(dc, key) => {
                let center = self.state.lock().unwrap().dcs.get(&bare_dc_id(dc)).cloned();
                if center.is_some_and(|d| d.destroy_confirmed_forgotten_key(key)) {
                    self.state.lock().unwrap().keys.remove(&bare_dc_id(dc));
                    self.state.lock().unwrap().notice(Notice::WriteKeys);
                }
                self.restart(Some(dc));
            }
            InstanceEvent::RequestConfig | InstanceEvent::RequestConfigIfOld => {
                self.request_config()
            }
            InstanceEvent::RequestCdnConfig => {
                self.request_cdn_config();
            }
            InstanceEvent::SyncHttpUnixtime => self.sync_http_unixtime(),
            InstanceEvent::RestartedByTimeout(dc) => {
                self.state.lock().unwrap().notice(Notice::Restarted(dc))
            }
            InstanceEvent::BadConfigurationError => {
                self.state.lock().unwrap().notice(Notice::BadConfiguration)
            }
        }
    }
    pub fn process_callback(&self, response: &Response) {
        let mut handler = self
            .state
            .lock()
            .unwrap()
            .callbacks
            .remove(&response.request_id)
            .unwrap_or_default();
        if handler.done.is_none() && handler.fail.is_none() {
            self.unregister_request(response.request_id);
            return;
        }
        self.sync_environment();
        let error = if response.reply.is_empty() {
            Some(Error::local("RESPONSE_PARSE_FAILED", "Empty response."))
        } else if response.reply[0] as u32 == 0x2144ca19 {
            Some(Error::from_reply(&response.reply))
        } else if handler.done.as_mut().is_some_and(|done| !done(response)) {
            Some(Error::local(
                "RESPONSE_PARSE_FAILED",
                "Response parse failed.",
            ))
        } else {
            None
        };
        let keep = error.is_some_and(|error| !self.rpc_error_occurred(response, &handler, &error));
        if keep {
            self.state
                .lock()
                .unwrap()
                .callbacks
                .insert(response.request_id, handler);
        } else {
            self.unregister_request(response.request_id);
        }
        self.sync_environment();
    }
    pub fn rpc_error_occurred(
        &self,
        response: &Response,
        handler: &ResponseHandler,
        error: &Error,
    ) -> bool {
        if error.is_default_handled_error()
            && handler
                .fail
                .as_ref()
                .is_some_and(|fail| fail(error, response))
        {
            return true;
        }
        if self.on_error_default(error, response) {
            return false;
        }
        if let Some(fail) = &handler.fail {
            fail(error, response);
        }
        true
    }
    fn on_error_default(&self, error: &Error, response: &Response) -> bool {
        let id = response.request_id;
        let route = self
            .state
            .lock()
            .unwrap()
            .routes
            .get(&id)
            .copied()
            .unwrap_or(0);
        let migration = [
            "PHONE_MIGRATE_",
            "FILE_MIGRATE_",
            "NETWORK_MIGRATE_",
            "USER_MIGRATE_",
        ]
        .iter()
        .find_map(|p| numeric_suffix(&error.kind, p));
        if let Some(dc) = migration {
            if route == 0 || dc == 0 {
                return false;
            }
            let target = if route < 0 {
                self.set_main_dc_id(dc);
                dc
            } else {
                shift_dc_id(dc, get_dc_id_shift(route))
            };
            self.state
                .lock()
                .unwrap()
                .routes
                .insert(id, if route < 0 { -target } else { target });
            self.resend(id);
            return true;
        }
        if error.kind == "MSG_WAIT_TIMEOUT" || error.kind == "MSG_WAIT_FAILED" {
            let handle = self.state.lock().unwrap().requests.get(&id).copied();
            let after =
                handle.and_then(|h| self.store.lock().unwrap().get(h).and_then(|r| r.after));
            let Some(after) = after else {
                return false;
            };
            let parent = self.store.lock().unwrap().get(after).map(|r| r.request_id);
            let parent_route =
                parent.and_then(|parent| self.state.lock().unwrap().routes.get(&parent).copied());
            if route == 0 {
                return false;
            }
            if parent_route != Some(route) {
                if let Some(h) = handle {
                    self.store.lock().unwrap().get_mut(h).unwrap().after = None;
                }
                self.resend(id);
            } else {
                self.state
                    .lock()
                    .unwrap()
                    .dependent
                    .insert(id, parent.unwrap());
            }
            return true;
        }
        let flood = numeric_suffix(&error.kind, "FLOOD_WAIT_");
        let premium = numeric_suffix(&error.kind, "FLOOD_PREMIUM_WAIT_");
        let slow = numeric_suffix(&error.kind, "SLOWMODE_WAIT_").filter(|n| *n < 3);
        if error.code < 0
            || error.code >= 500
            || flood.is_some()
            || premium.is_some()
            || slow.is_some()
        {
            if id == 0 {
                return false;
            }
            let mut s = self.state.lock().unwrap();
            let secs = if error.code < 0 || error.code >= 500 {
                let delay = s.delays.entry(id).or_insert(0);
                *delay = if *delay == 0 {
                    1
                } else if *delay > 60 {
                    *delay
                } else {
                    *delay * 2
                };
                *delay
            } else {
                flood.or(premium).or(slow).unwrap()
            };
            let at = s.now + i64::from(secs) * 1000 + 10;
            let index = s
                .delayed
                .iter()
                .position(|(_, time)| *time > at)
                .unwrap_or(s.delayed.len());
            if !s.delayed.iter().take(index + 1).any(|(r, _)| *r == id) {
                s.delayed.insert(index, (id, at));
            }
            if premium.is_some() {
                s.notice(Notice::NonPremiumDelayed(id));
            }
            return true;
        }
        if error.kind == "CONNECTION_NOT_INITED" || error.kind == "CONNECTION_LAYER_INVALID" {
            let handle = self.state.lock().unwrap().requests.get(&id).copied();
            if let Some(h) = handle {
                self.store.lock().unwrap().get_mut(h).unwrap().needs_layer = true;
                if let Some(session) = self.session(route.abs()) {
                    session.data().dc.set_connection_inited(false);
                }
                self.resend(id);
                return route != 0;
            }
        }
        if error.kind == "CONNECTION_LANG_CODE_INVALID" {
            self.state.lock().unwrap().notice(Notice::LanguageReset);
        }
        if error.kind == "FROZEN_METHOD_INVALID" {
            self.state.lock().unwrap().notice(Notice::Frozen);
        }
        let bad_guest = error.code == 400 && error.kind == "FILE_ID_INVALID";
        let first_bad = bad_guest && !self.state.lock().unwrap().bad_guest.contains(&id);
        if ((error.code == 401 && error.kind != "AUTH_KEY_PERM_EMPTY") || first_bad)
            && self.authorize_guest(id, route, bad_guest)
        {
            return true;
        }
        if bad_guest {
            self.state.lock().unwrap().bad_guest.remove(&id);
        }
        if error.code == 401 && error.kind != "AUTH_KEY_PERM_EMPTY" {
            let (mut h, epoch) = {
                let mut s = self.state.lock().unwrap();
                (s.global_fail.take(), s.handler_epochs[1])
            };
            if let Some(f) = &mut h {
                f(error, response);
            }
            let mut s = self.state.lock().unwrap();
            if s.handler_epochs[1] == epoch {
                s.global_fail = h;
            }
        }
        false
    }
}
impl SessionEnvironment for Instance {
    fn dc_options(&self) -> DcOptions {
        self.state.lock().unwrap().config.dc_options.clone()
    }
    fn is_test_mode(&self) -> bool {
        self.state.lock().unwrap().config.is_test_mode()
    }
    fn is_keys_destroyer(&self) -> bool {
        self.state.lock().unwrap().mode == Mode::KeysDestroyer
    }
    fn device_model(&self) -> String {
        self.state.lock().unwrap().device.clone()
    }
    fn system_version(&self) -> String {
        self.state.lock().unwrap().system.clone()
    }
    fn system_lang_code(&self) -> String {
        self.state.lock().unwrap().languages[0].clone()
    }
    fn cloud_lang_code(&self) -> String {
        self.state.lock().unwrap().languages[1].clone()
    }
    fn lang_pack_name(&self) -> String {
        self.state.lock().unwrap().languages[2].clone()
    }
    fn proxy_settings(&self) -> ProxySettings {
        self.state.lock().unwrap().proxy.clone()
    }
    fn has_callback(&self, id: i32) -> bool {
        self.state.lock().unwrap().callbacks.contains_key(&id)
    }
    fn subscribe(&self) -> mpsc::UnboundedReceiver<EnvironmentChange> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.environment.changes.lock().unwrap().push(tx);
        rx
    }
    fn emit(&self, event: InstanceEvent) {
        self.environment.events.lock().unwrap().push_back(event);
    }
}

#[cfg(test)]
mod tests;

impl Instance {
    pub fn export_done(&self, result: &tg_scheme::AuthExportedAuthorization, response: &Response) {
        let target = self
            .state
            .lock()
            .unwrap()
            .exports
            .remove(&response.request_id);
        let Some(target) = target else {
            return;
        };
        let data = result.c_auth_exported_authorization();
        let owner = self.downgrade();
        let fail = self.downgrade();
        self.send(
            &tg_scheme::AuthImportAuthorization::new(data.id, data.bytes.clone()),
            ResponseHandler {
                done: Some(Box::new(move |response| {
                    let mut reader = tg_tl::Reader::new(&response.reply);
                    if <tg_scheme::AuthAuthorization as tg_tl::Deserialize>::read(&mut reader)
                        .is_err()
                    {
                        return false;
                    }
                    owner.import_done(response);
                    true
                })),
                fail: Some(Box::new(move |error, response| {
                    fail.import_fail(error, response)
                })),
            },
            target,
            0,
            0,
            0,
        );
    }
    pub fn export_fail(&self, error: &Error, response: &Response) -> bool {
        if error.is_default_handled_error() {
            return false;
        }
        let mut s = self.state.lock().unwrap();
        if let Some(target) = s.exports.get(&response.request_id).copied() {
            s.waiters.entry(bare_dc_id(target)).or_default().clear();
        }
        true
    }
    pub fn import_fail(&self, error: &Error, _response: &Response) -> bool {
        !error.is_default_handled_error()
    }
    pub fn import_done(&self, response: &Response) {
        let route = self
            .state
            .lock()
            .unwrap()
            .routes
            .get(&response.request_id)
            .copied();
        let Some(route) = route else {
            return;
        };
        let dc = bare_dc_id(route);
        let waiting = self
            .state
            .lock()
            .unwrap()
            .waiters
            .entry(dc)
            .or_default()
            .drain(..)
            .collect::<Vec<_>>();
        for id in waiting {
            let old = self.state.lock().unwrap().routes.get(&id).copied();
            if let Some(old) = old {
                let target = if old < 0 {
                    self.set_main_dc_id(dc);
                    -dc
                } else {
                    shift_dc_id(dc, get_dc_id_shift(old))
                };
                self.state.lock().unwrap().routes.insert(id, target);
                self.resend(id);
            }
        }
    }
    fn authorize_guest(&self, id: i32, route: i32, bad_guest: bool) -> bool {
        let target = bare_dc_id(route.abs());
        if target == 0 || target == self.main_dc_id() {
            return false;
        }
        let first = self
            .state
            .lock()
            .unwrap()
            .waiters
            .entry(target)
            .or_default()
            .is_empty();
        if first {
            let owner = self.downgrade();
            let fail = self.downgrade();
            let export = self.send(
                &tg_scheme::AuthExportAuthorization::new(target),
                ResponseHandler {
                    done: Some(Box::new(move |response| {
                        let mut reader = tg_tl::Reader::new(&response.reply);
                        let Ok(result) =
                            <tg_scheme::AuthExportedAuthorization as tg_tl::Deserialize>::read(
                                &mut reader,
                            )
                        else {
                            return false;
                        };
                        owner.export_done(&result, response);
                        true
                    })),
                    fail: Some(Box::new(move |error, response| {
                        fail.export_fail(error, response)
                    })),
                },
                0,
                0,
                0,
                0,
            );
            self.state
                .lock()
                .unwrap()
                .exports
                .insert(export, route.abs());
        }
        let mut s = self.state.lock().unwrap();
        s.waiters.entry(target).or_default().push(id);
        if bad_guest {
            s.bad_guest.insert(id);
        }
        true
    }
    pub fn add_keys_for_destroy(&self, keys: Vec<Arc<AuthKey>>) {
        assert!(self.is_keys_destroyer());
        for key in keys {
            let mut id = crate::facade::destroy_key_next_dc_id(key.dc_id());
            {
                let mut s = self.state.lock().unwrap();
                while s.keys.contains_key(&id) {
                    id = crate::facade::destroy_key_next_dc_id(id);
                }
                s.keys.insert(id, key.clone());
                s.dcs
                    .insert(id, Arc::new(Dcenter::new(bare_dc_id(id), Some(key))));
            }
            self.ensure_session(id);
            self.schedule_key_destroy(id);
        }
    }
    pub fn schedule_key_destroy(&self, id: i32) {
        if self.dc_options().dc_type(id) == crate::mtproto_dc_options::DcType::Cdn {
            self.perform_key_destroy(id);
        } else {
            let owner = self.downgrade();
            let fail = self.downgrade();
            self.send(
                &tg_scheme::AuthLogOut::new(),
                ResponseHandler {
                    done: Some(Box::new(move |_| {
                        owner.perform_key_destroy(id);
                        true
                    })),
                    fail: Some(Box::new(move |error, _| {
                        if error.is_default_handled_error() {
                            false
                        } else {
                            fail.perform_key_destroy(id);
                            true
                        }
                    })),
                },
                id,
                0,
                0,
                0,
            );
        }
    }
    pub fn perform_key_destroy(&self, id: i32) {
        let owner = self.downgrade();
        let fail = self.downgrade();
        self.send(
            &tg_scheme::DestroyAuthKey::new(),
            ResponseHandler {
                done: Some(Box::new(move |response| {
                    let mut reader = tg_tl::Reader::new(&response.reply);
                    let Ok(result) =
                        <tg_scheme::DestroyAuthKeyRes as tg_tl::Deserialize>::read(&mut reader)
                    else {
                        return false;
                    };
                    if matches!(result, tg_scheme::DestroyAuthKeyRes::DestroyAuthKeyFail(_)) {
                        owner.kill_session(id);
                    }
                    owner.emit(InstanceEvent::KeyWasPossiblyDestroyed(id));
                    true
                })),
                fail: Some(Box::new(move |_, _| {
                    fail.emit(InstanceEvent::KeyWasPossiblyDestroyed(id));
                    true
                })),
            },
            id,
            0,
            0,
            0,
        );
    }
    pub fn shutdown(&self) {
        let handles = {
            let mut s = self.state.lock().unwrap();
            s.callbacks.clear();
            s.spawns.clear();
            s.requests.clear();
            s.routes.clear();
            std::mem::take(&mut s.sessions)
        };
        for (_, h) in handles {
            h.send(SessionCommand::Kill);
        }
    }
}

impl Instance {
    pub fn add_temporary_endpoint(
        &self,
        id: i32,
        endpoint: &crate::special_config_request::Endpoint,
    ) {
        let mut s = self.state.lock().unwrap();
        s.config
            .dc_options
            .construct_add_one(crate::mtproto_dc_options::Endpoint {
                id,
                flags: 4 | if endpoint.secret.is_empty() { 0 } else { 1024 },
                ip: endpoint.ip.clone(),
                port: endpoint.port,
                secret: endpoint.secret.clone(),
            });
        drop(s);
        self.sync_environment();
        self.broadcast(EnvironmentChange::DcOptions(id));
    }
    pub fn apply_config(&self, result: &tg_scheme::Config, special: bool) {
        let data = result.c_config();
        let mut s = self.state.lock().unwrap();
        if special {
            if !data.dc_options.is_empty() {
                s.config.dc_options.set_from_list(&data.dc_options);
            }
        } else {
            s.config.apply(data);
            s.last_config = s.now;
            s.config_expires =
                s.now + (i64::from(data.expires) - i64::from(tg_base::unixtime::now())) * 1000;
            s.config_loading = false;
            s.notice(Notice::ConfigApplied {
                suggested_lang: String::from_utf8_lossy(
                    data.vsuggested_lang_code().map_or(&[], |v| v.as_slice()),
                )
                .into_owned(),
                lang_version: data.vlang_pack_version().unwrap_or(0),
                base_lang_version: data.vbase_lang_pack_version().unwrap_or(0),
                autoupdate_prefix: data
                    .vautoupdate_url_prefix()
                    .map(|p| String::from_utf8_lossy(p).into_owned()),
            });
        }
        let ids = s.sessions.keys().copied().collect::<Vec<_>>();
        drop(s);
        self.sync_environment();
        for id in ids {
            self.broadcast(EnvironmentChange::DcOptions(bare_dc_id(id)));
        }
    }
}

pub struct InstanceTimers {
    instance: Instance,
    host: std::rc::Rc<tg_base::timer::TimerHost>,
    timer: tg_base::timer::Timer,
    due: std::rc::Rc<std::cell::Cell<bool>>,
}
impl InstanceTimers {
    pub fn new(instance: Instance, host: std::rc::Rc<tg_base::timer::TimerHost>) -> Self {
        let due = std::rc::Rc::new(std::cell::Cell::new(false));
        let callback = due.clone();
        let timer = tg_base::timer::Timer::in_host(
            &host,
            Some(std::rc::Rc::new(move || callback.set(true))),
        );
        Self {
            instance,
            host,
            timer,
            due,
        }
    }
    pub fn process(&self) {
        self.host.process();
        if self.due.replace(false) {
            self.instance.check_delayed_requests(self.host.now());
        }
        if let Some(deadline) = self.instance.next_delayed_deadline() {
            let wait = (deadline - self.host.now()).max(0);
            if !self.timer.is_active() || self.timer.remaining_time() != wait {
                self.timer.call_once(wait);
            }
        } else {
            self.timer.cancel();
        }
    }
}

impl Instance {
    pub fn get_dc_by_id(&self, id: i32) -> Arc<Dcenter> {
        self.ensure_session(id);
        let s = self.state.lock().unwrap();
        if let Some(dc) = s.dcs.get(&id) {
            return dc.clone();
        }
        let real = crate::facade::get_real_id_from_temporary_dc_id(id);
        s.dcs[&if real != 0 { real } else { bare_dc_id(id) }].clone()
    }
    pub fn set_device_model(&self, device: String) {
        self.state.lock().unwrap().device = device;
        self.sync_environment();
        self.re_init_connection(self.main_dc_id());
    }
    pub fn set_languages(&self, system: String, cloud: String, pack: String) {
        self.state.lock().unwrap().languages = [system, cloud, pack];
        self.sync_environment();
        self.re_init_connection(self.main_dc_id());
    }
    pub fn set_proxy_settings(&self, proxy: ProxySettings) {
        self.state.lock().unwrap().proxy = proxy;
        self.sync_environment();
        self.restart(None);
    }
    pub fn apply_domain_ips(&self, host: &str, ips: &[String], expires: i64) {
        let mut s = self.state.lock().unwrap();
        let proxy = &mut s.proxy.selected;
        if proxy.host != host || !proxy.try_custom_resolve() {
            return;
        }
        proxy.resolved_expire_at = expires;
        let mut copy = ips.to_vec();
        proxy.resolved_ips.retain(|ip| {
            if let Some(index) = copy.iter().position(|x| x == ip) {
                copy.remove(index);
                true
            } else {
                false
            }
        });
        proxy.resolved_ips.extend(copy);
        let handles = s.sessions.values().cloned().collect::<Vec<_>>();
        drop(s);
        self.sync_environment();
        for h in handles {
            h.data().refresh_options();
        }
    }
    pub fn set_good_proxy_domain(&self, host: &str, ip: &str) {
        let mut s = self.state.lock().unwrap();
        let proxy = &mut s.proxy.selected;
        if proxy.host != host || !proxy.try_custom_resolve() {
            return;
        }
        if let Some(index) = proxy.resolved_ips.iter().position(|x| x == ip) {
            proxy.resolved_ips[..=index].rotate_right(1);
        }
        drop(s);
        self.sync_environment();
    }
    pub fn send_anything(&self, dc: i32, wait: i64) {
        self.ensure_session(dc);
        if let Some(session) = self.session(dc) {
            session.send(SessionCommand::SendAnything(wait));
        }
    }
    pub fn unpaused(&self) {
        let sessions = self
            .state
            .lock()
            .unwrap()
            .sessions
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for session in sessions {
            session.send(SessionCommand::Unpaused);
        }
    }
    pub fn logout(&self, done: impl Fn() + Send + Sync + 'static) {
        let done = Arc::new(done);
        let fail = done.clone();
        self.send(
            &tg_scheme::AuthLogOut::new(),
            ResponseHandler {
                done: Some(Box::new(move |_| {
                    done();
                    true
                })),
                fail: Some(Box::new(move |_, _| {
                    fail();
                    true
                })),
            },
            0,
            0,
            0,
            0,
        );
        let ids = self
            .state
            .lock()
            .unwrap()
            .keys
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for dc in ids {
            if dc == self.main_dc_id()
                || self.dc_options().dc_type(dc) == crate::mtproto_dc_options::DcType::Cdn
            {
                continue;
            }
            let target = crate::facade::logout_dc_id(dc);
            let owner = self.downgrade();
            let fail = self.downgrade();
            self.send(
                &tg_scheme::AuthLogOut::new(),
                ResponseHandler {
                    done: Some(Box::new(move |_| {
                        owner.kill_session(target);
                        true
                    })),
                    fail: Some(Box::new(move |_, _| {
                        fail.kill_session(target);
                        true
                    })),
                },
                target,
                0,
                0,
                0,
            );
        }
    }
}

impl Instance {
    fn collect_requests(&self) {
        let mut s = self.state.lock().unwrap();
        let mut store = self.store.lock().unwrap();
        loop {
            let referenced = s
                .requests
                .values()
                .chain(s.retired.iter())
                .filter_map(|handle| store.get(*handle).and_then(|r| r.after))
                .collect::<BTreeSet<_>>();
            let victims = s
                .retired
                .difference(&referenced)
                .copied()
                .collect::<Vec<_>>();
            if victims.is_empty() {
                break;
            }
            for h in victims {
                s.retired.remove(&h);
                store.remove(h);
            }
        }
    }
    pub fn with_config<T>(&self, read: impl FnOnce(&Config) -> T) -> T {
        read(&self.state.lock().unwrap().config)
    }
    pub fn config_values(&self) -> crate::mtproto_config::ConfigFields {
        self.state.lock().unwrap().config.values().clone()
    }
    pub fn environment(&self) -> Environment {
        self.state.lock().unwrap().config.environment()
    }
    pub fn dc_persistent_key_changed(&self, dc: i32, key: Option<Arc<AuthKey>>) {
        self.dc_temporary_key_changed(dc);
        if crate::facade::is_temporary_dc_id(dc) {
            return;
        }
        let mut s = self.state.lock().unwrap();
        let same = match (s.keys.get(&dc), key.as_ref()) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        if same {
            return;
        }
        if let Some(key) = key {
            s.keys.insert(dc, key);
        } else {
            s.keys.remove(&dc);
        }
        s.notice(Notice::WriteKeys);
    }
}

struct EnvironmentSnapshot {
    dc_options: DcOptions,
    test: bool,
    destroyer: bool,
    device: String,
    system: String,
    languages: [String; 3],
    proxy: ProxySettings,
    callbacks: BTreeSet<i32>,
}
struct EnvironmentBridge {
    snapshot: Mutex<EnvironmentSnapshot>,
    events: Arc<Mutex<VecDeque<InstanceEvent>>>,
    changes: Mutex<Vec<mpsc::UnboundedSender<EnvironmentChange>>>,
    proxy_updates:
        Mutex<Vec<mpsc::UnboundedSender<crate::details::mtproto_domain_resolver::Resolved>>>,
}
impl EnvironmentBridge {
    fn new(fields: &Fields, mode: Mode) -> Self {
        Self {
            snapshot: Mutex::new(EnvironmentSnapshot {
                dc_options: fields.config.dc_options.clone(),
                test: fields.config.is_test_mode(),
                destroyer: mode == Mode::KeysDestroyer,
                device: fields.device_model.clone(),
                system: fields.system_version.clone(),
                languages: ["en".into(), "en".into(), "tdesktop".into()],
                proxy: ProxySettings::default(),
                callbacks: BTreeSet::new(),
            }),
            events: Arc::new(Mutex::new(VecDeque::new())),
            changes: Mutex::new(vec![]),
            proxy_updates: Mutex::new(vec![]),
        }
    }
}
impl SessionEnvironment for EnvironmentBridge {
    fn dc_options(&self) -> DcOptions {
        self.snapshot.lock().unwrap().dc_options.clone()
    }
    fn is_test_mode(&self) -> bool {
        self.snapshot.lock().unwrap().test
    }
    fn is_keys_destroyer(&self) -> bool {
        self.snapshot.lock().unwrap().destroyer
    }
    fn device_model(&self) -> String {
        self.snapshot.lock().unwrap().device.clone()
    }
    fn system_version(&self) -> String {
        self.snapshot.lock().unwrap().system.clone()
    }
    fn system_lang_code(&self) -> String {
        self.snapshot.lock().unwrap().languages[0].clone()
    }
    fn cloud_lang_code(&self) -> String {
        self.snapshot.lock().unwrap().languages[1].clone()
    }
    fn lang_pack_name(&self) -> String {
        self.snapshot.lock().unwrap().languages[2].clone()
    }
    fn proxy_settings(&self) -> ProxySettings {
        self.snapshot.lock().unwrap().proxy.clone()
    }
    fn has_callback(&self, id: i32) -> bool {
        self.snapshot.lock().unwrap().callbacks.contains(&id)
    }
    fn subscribe(&self) -> mpsc::UnboundedReceiver<EnvironmentChange> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.changes.lock().unwrap().push(tx);
        rx
    }
    fn emit(&self, event: InstanceEvent) {
        self.events.lock().unwrap().push_back(event);
    }
}
impl Instance {
    pub fn session_environment(&self) -> Arc<dyn SessionEnvironment> {
        self.sync_environment();
        self.environment.clone()
    }
    fn sync_environment(&self) {
        let s = self.state.lock().unwrap();
        let mut snapshot = self.environment.snapshot.lock().unwrap();
        snapshot.dc_options = s.config.dc_options.clone();
        snapshot.device = s.device.clone();
        snapshot.system = s.system.clone();
        snapshot.languages = s.languages.clone();
        snapshot.proxy = s.proxy.clone();
        snapshot.callbacks = s.callbacks.keys().copied().collect();
    }
}
impl Drop for State {
    fn drop(&mut self) {
        for session in self.sessions.values() {
            session.send(SessionCommand::Kill);
        }
    }
}

#[derive(Clone)]
pub(crate) struct WeakInstance {
    state: std::sync::Weak<Mutex<State>>,
    store: std::sync::Weak<Mutex<RequestStore>>,
    environment: std::sync::Weak<EnvironmentBridge>,
}
impl Instance {
    pub(crate) fn downgrade(&self) -> WeakInstance {
        WeakInstance {
            state: Arc::downgrade(&self.state),
            store: Arc::downgrade(&self.store),
            environment: Arc::downgrade(&self.environment),
        }
    }
}
impl WeakInstance {
    pub(crate) fn upgrade(&self) -> Option<Instance> {
        Some(Instance {
            state: self.state.upgrade()?,
            store: self.store.upgrade()?,
            environment: self.environment.upgrade()?,
        })
    }
    fn import_done(&self, response: &Response) {
        if let Some(i) = self.upgrade() {
            i.import_done(response);
        }
    }
    fn import_fail(&self, error: &Error, response: &Response) -> bool {
        self.upgrade()
            .is_none_or(|i| i.import_fail(error, response))
    }
    fn export_done(&self, result: &tg_scheme::AuthExportedAuthorization, response: &Response) {
        if let Some(i) = self.upgrade() {
            i.export_done(result, response);
        }
    }
    fn export_fail(&self, error: &Error, response: &Response) -> bool {
        self.upgrade()
            .is_none_or(|i| i.export_fail(error, response))
    }
    fn perform_key_destroy(&self, id: i32) {
        if let Some(i) = self.upgrade() {
            i.perform_key_destroy(id);
        }
    }
    fn kill_session(&self, id: i32) {
        if let Some(i) = self.upgrade() {
            i.kill_session(id);
        }
    }
    fn emit(&self, event: InstanceEvent) {
        if let Some(i) = self.upgrade() {
            i.emit(event);
        }
    }
}

impl Instance {
    fn take_service_requests(&self) -> Vec<Notice> {
        let mut s = self.state.lock().unwrap();
        let mut result = vec![];
        s.pub_events.retain(|notice| {
            if matches!(notice, Notice::ConfigNeeded | Notice::HttpTimeNeeded) {
                result.push(notice.clone());
                false
            } else {
                true
            }
        });
        result
    }
}

impl Instance {
    pub fn request_config_if_old(&self) {
        let old = {
            let s = self.state.lock().unwrap();
            s.now - s.last_config
                >= if s.config.fields.blocked_mode {
                    8000
                } else {
                    120000
                }
        };
        if old {
            self.request_config();
        }
    }
    pub fn request_config_if_expired(&self) -> Option<i64> {
        let wait = {
            let s = self.state.lock().unwrap();
            s.config_expires - s.now
        };
        if wait > 0 {
            Some(wait.min(3600000))
        } else {
            self.request_config();
            None
        }
    }
    pub fn set_user_phone(&self, phone: String) {
        self.state.lock().unwrap().phone = phone;
    }
    pub fn user_phone(&self) -> String {
        self.state.lock().unwrap().phone.clone()
    }
    pub fn request_cdn_config(&self) -> i32 {
        if self.state.lock().unwrap().cdn_request != 0 || self.main_dc_id() == -1 {
            return 0;
        }
        let weak = self.downgrade();
        let id = self.send(
            &tg_scheme::HelpGetCdnConfig::new(),
            ResponseHandler {
                done: Some(Box::new(move |response| {
                    let mut reader = tg_tl::Reader::new(&response.reply);
                    let Ok(result) =
                        <tg_scheme::CdnConfig as tg_tl::Deserialize>::read(&mut reader)
                    else {
                        return false;
                    };
                    if let Some(instance) = weak.upgrade() {
                        let mut s = instance.state.lock().unwrap();
                        s.cdn_request = 0;
                        s.config.dc_options.set_cdn_config(result.c_cdn_config());
                        drop(s);
                        instance.sync_environment();
                        instance.broadcast(EnvironmentChange::CdnConfig);
                    }
                    true
                })),
                fail: None,
            },
            0,
            0,
            0,
            0,
        );
        self.state.lock().unwrap().cdn_request = id;
        id
    }
    pub fn sync_http_unixtime(&self) {
        if tg_base::unixtime::http_valid() {
            return;
        }
        let mut s = self.state.lock().unwrap();
        if !s.http_time_loading {
            s.http_time_loading = true;
            s.notice(Notice::HttpTimeNeeded);
        }
    }
}

impl Instance {
    pub fn resolve_proxy_domain(&self, host: &str, now: i64, random: &mut impl Random) {
        let work = self
            .state
            .lock()
            .unwrap()
            .resolver
            .resolve(host, now, random);
        self.state.lock().unwrap().resolve_work.extend(work);
        self.apply_resolved_domains();
    }
    pub fn poll_domain_requests(
        &self,
        now: i64,
        random: &mut impl Random,
    ) -> VecDeque<crate::details::mtproto_domain_resolver::Request> {
        let mut s = self.state.lock().unwrap();
        let work = s.resolver.poll(now, random);
        s.resolve_work.extend(work);
        std::mem::take(&mut s.resolve_work)
    }
    pub fn domain_request_finished(
        &self,
        request: &crate::details::mtproto_domain_resolver::Request,
        bytes: &[u8],
        now: i64,
    ) {
        self.state
            .lock()
            .unwrap()
            .resolver
            .request_finished(request, bytes, now);
        self.apply_resolved_domains();
    }
    fn apply_resolved_domains(&self) {
        let results = std::mem::take(&mut self.state.lock().unwrap().resolver.resolved);
        for result in results {
            self.apply_domain_ips(&result.domain, &result.ips, result.expire_at);
            self.environment
                .proxy_updates
                .lock()
                .unwrap()
                .retain(|tx| tx.send(result.clone()).is_ok());
        }
    }
    fn subscribe_proxy_domains(
        &self,
    ) -> mpsc::UnboundedReceiver<crate::details::mtproto_domain_resolver::Resolved> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.environment.proxy_updates.lock().unwrap().push(tx);
        rx
    }
}

impl Instance {
    pub fn next_domain_deadline(&self) -> Option<i64> {
        self.state.lock().unwrap().resolver.next_deadline()
    }
}

fn numeric_suffix(kind: &str, prefix: &str) -> Option<i32> {
    let n = kind.strip_prefix(prefix)?;
    if n.is_empty() || !n.bytes().all(|c| c.is_ascii_digit()) {
        None
    } else {
        Some(n.parse().unwrap_or(0))
    }
}

impl State {
    fn notice(&mut self, notice: Notice) {
        self.notice_subscribers
            .retain(|tx| tx.send(notice.clone()).is_ok());
        self.pub_events.push_back(notice);
    }
}
pub struct NoticeStream<T> {
    events: mpsc::UnboundedReceiver<Notice>,
    initial: Option<T>,
    filter: fn(&Notice) -> Option<T>,
}
impl<T> NoticeStream<T> {
    pub fn try_recv(&mut self) -> Result<T, mpsc::error::TryRecvError> {
        if let Some(initial) = self.initial.take() {
            return Ok(initial);
        }
        loop {
            let event = self.events.try_recv()?;
            if let Some(value) = (self.filter)(&event) {
                return Ok(value);
            }
        }
    }
    pub async fn recv(&mut self) -> Option<T> {
        if let Some(initial) = self.initial.take() {
            return Some(initial);
        }
        while let Some(event) = self.events.recv().await {
            if let Some(value) = (self.filter)(&event) {
                return Some(value);
            }
        }
        None
    }
}
impl Instance {
    pub fn subscribe_notices(&self) -> mpsc::UnboundedReceiver<Notice> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.state.lock().unwrap().notice_subscribers.push(tx);
        rx
    }
    fn notice_stream<T>(
        &self,
        initial: Option<T>,
        filter: fn(&Notice) -> Option<T>,
    ) -> NoticeStream<T> {
        NoticeStream {
            events: self.subscribe_notices(),
            initial,
            filter,
        }
    }
    pub fn main_dc_id_value(&self) -> NoticeStream<i32> {
        self.notice_stream(Some(self.main_dc_id()), |notice| {
            if let Notice::MainDc(id) = notice {
                Some(*id)
            } else {
                None
            }
        })
    }
    pub fn write_keys_requests(&self) -> NoticeStream<()> {
        self.notice_stream(None, |notice| {
            matches!(notice, Notice::WriteKeys).then_some(())
        })
    }
    pub fn all_keys_destroyed(&self) -> NoticeStream<()> {
        self.notice_stream(None, |notice| {
            matches!(notice, Notice::AllKeysDestroyed).then_some(())
        })
    }
    pub fn temporary_key_changes(&self) -> NoticeStream<i32> {
        self.notice_stream(None, |notice| {
            if let Notice::TemporaryKeyChanged(id) = notice {
                Some(*id)
            } else {
                None
            }
        })
    }
    pub fn restarts_by_timeout(&self) -> NoticeStream<i32> {
        self.notice_stream(None, |notice| {
            if let Notice::Restarted(id) = notice {
                Some(*id)
            } else {
                None
            }
        })
    }
    pub fn non_premium_delayed_requests(&self) -> NoticeStream<i32> {
        self.notice_stream(None, |notice| {
            if let Notice::NonPremiumDelayed(id) = notice {
                Some(*id)
            } else {
                None
            }
        })
    }
    pub fn frozen_error_received(&self) -> NoticeStream<()> {
        self.notice_stream(None, |notice| {
            matches!(notice, Notice::Frozen).then_some(())
        })
    }
    pub fn dc_temporary_key_changed(&self, dc: i32) {
        self.broadcast(EnvironmentChange::TemporaryKey(dc));
        self.state
            .lock()
            .unwrap()
            .notice(Notice::TemporaryKeyChanged(dc));
    }
}
