//! Request ownership, DC routing and main-thread callback dispatch.
pub mod network;
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
    routes: BTreeMap<i32, i32>,
    callbacks: BTreeMap<i32, ResponseHandler>,
    events: VecDeque<InstanceEvent>,
    changes: Vec<mpsc::UnboundedSender<EnvironmentChange>>,
    delayed: VecDeque<(i32, i64)>,
    delays: BTreeMap<i32, i32>,
    dependent: BTreeMap<i32, i32>,
    bad_guest: BTreeSet<i32>,
    waiters: BTreeMap<i32, Vec<i32>>,
    exports: BTreeMap<i32, i32>,
    updates: Option<UpdateHandler>,
    global_fail: Option<GlobalFailHandler>,
    state_changed: Option<StateHandler>,
    reset: Option<ResetHandler>,
    pub_events: VecDeque<Notice>,
    now: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    WriteKeys,
    AllKeysDestroyed,
    MainDc(i32),
    Restarted(i32),
    NonPremiumDelayed(i32),
    Frozen,
    LanguageReset,
    BadConfiguration,
    ConfigNeeded,
    CdnConfigNeeded,
    HttpTimeNeeded,
}
#[derive(Clone)]
pub struct Instance {
    state: Arc<Mutex<State>>,
    pub store: Arc<Mutex<RequestStore>>,
}
impl Instance {
    pub fn new(mode: Mode, fields: Fields) -> Self {
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
        Self {
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
                routes: BTreeMap::new(),
                callbacks: BTreeMap::new(),
                events: VecDeque::new(),
                changes: vec![],
                delayed: VecDeque::new(),
                delays: BTreeMap::new(),
                dependent: BTreeMap::new(),
                bad_guest: BTreeSet::new(),
                waiters: BTreeMap::new(),
                exports: BTreeMap::new(),
                updates: None,
                global_fail: None,
                state_changed: None,
                reset: None,
                pub_events: VecDeque::new(),
                now: 0,
            })),
        }
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
        s.pub_events.push_back(Notice::MainDc(id));
        s.pub_events.push_back(Notice::WriteKeys);
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
                .filter_map(|(rid, _)| s.requests.get(rid).copied())
                .collect()
        };
        for handle in pending {
            self.send_handle(id, handle, 0);
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
            s.routes.insert(id, if dc == 0 { -target } else { target });
            if callbacks.done.is_some() || callbacks.fail.is_some() {
                s.callbacks.insert(id, callbacks);
            }
            (handle, blocked)
        };
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
            self.store.lock().unwrap().remove(h);
        }
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
        if !self.is_keys_destroyer() {
            self.state
                .lock()
                .unwrap()
                .pub_events
                .push_back(Notice::ConfigNeeded);
        }
    }
    pub fn process(&self, now: i64) {
        assert!(tg_runtime::main_queue().is_main_thread());
        self.state.lock().unwrap().now = now;
        loop {
            let event = self.state.lock().unwrap().events.pop_front();
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
        self.state.lock().unwrap().updates = h;
    }
    pub fn set_global_fail_handler(&self, h: Option<GlobalFailHandler>) {
        self.state.lock().unwrap().global_fail = h;
    }
    pub fn set_state_changed_handler(&self, h: Option<StateHandler>) {
        self.state.lock().unwrap().state_changed = h;
    }
    pub fn set_session_reset_handler(&self, h: Option<ResetHandler>) {
        self.state.lock().unwrap().reset = h;
    }
    pub fn clear_global_handlers(&self) {
        let mut s = self.state.lock().unwrap();
        s.updates = None;
        s.global_fail = None;
        s.state_changed = None;
        s.reset = None;
    }
    fn broadcast(&self, change: EnvironmentChange) {
        self.state
            .lock()
            .unwrap()
            .changes
            .retain(|tx| tx.send(change.clone()).is_ok());
    }
    fn process_event(&self, event: InstanceEvent) {
        match event {
            InstanceEvent::Callback(r) => self.process_callback(&r),
            InstanceEvent::Update(r) => {
                let mut h = self.state.lock().unwrap().updates.take();
                if let Some(f) = &mut h {
                    f(&r);
                }
                self.state.lock().unwrap().updates = h;
            }
            InstanceEvent::StateChange(dc, state) => {
                let mut h = self.state.lock().unwrap().state_changed.take();
                if let Some(f) = &mut h {
                    f(dc, state);
                }
                self.state.lock().unwrap().state_changed = h;
            }
            InstanceEvent::SessionReset(dc) => {
                let mut h = self.state.lock().unwrap().reset.take();
                if let Some(f) = &mut h {
                    f(dc);
                }
                self.state.lock().unwrap().reset = h;
            }
            InstanceEvent::DcPersistentKeyChanged(dc, key) => {
                self.state.lock().unwrap().keys.insert(dc, key);
                self.broadcast(EnvironmentChange::TemporaryKey(dc));
                self.state
                    .lock()
                    .unwrap()
                    .pub_events
                    .push_back(Notice::WriteKeys);
            }
            InstanceEvent::DcTemporaryKeyChanged(dc) => {
                self.broadcast(EnvironmentChange::TemporaryKey(dc))
            }
            InstanceEvent::KeyWasPossiblyDestroyed(dc) => {
                let mut s = self.state.lock().unwrap();
                s.dcs.remove(&dc);
                s.keys.remove(&dc);
                if s.dcs.is_empty() {
                    s.pub_events.push_back(Notice::AllKeysDestroyed);
                }
                drop(s);
                self.schedule_session_destroy(dc);
            }
            InstanceEvent::KeyDestroyedOnServer(dc, key) => {
                let center = self.state.lock().unwrap().dcs.get(&bare_dc_id(dc)).cloned();
                if center.is_some_and(|d| d.destroy_confirmed_forgotten_key(key)) {
                    self.state.lock().unwrap().keys.remove(&bare_dc_id(dc));
                    self.state
                        .lock()
                        .unwrap()
                        .pub_events
                        .push_back(Notice::WriteKeys);
                }
                self.restart(Some(dc));
            }
            InstanceEvent::RequestConfig | InstanceEvent::RequestConfigIfOld => {
                self.request_config()
            }
            InstanceEvent::RequestCdnConfig => self
                .state
                .lock()
                .unwrap()
                .pub_events
                .push_back(Notice::CdnConfigNeeded),
            InstanceEvent::SyncHttpUnixtime => self
                .state
                .lock()
                .unwrap()
                .pub_events
                .push_back(Notice::HttpTimeNeeded),
            InstanceEvent::RestartedByTimeout(dc) => self
                .state
                .lock()
                .unwrap()
                .pub_events
                .push_back(Notice::Restarted(dc)),
            InstanceEvent::BadConfigurationError => self
                .state
                .lock()
                .unwrap()
                .pub_events
                .push_back(Notice::BadConfiguration),
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
        .find_map(|p| {
            error
                .kind
                .strip_prefix(p)
                .and_then(|x| x.parse::<i32>().ok())
        });
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
        let flood = error
            .kind
            .strip_prefix("FLOOD_WAIT_")
            .and_then(|x| x.parse::<i32>().ok());
        let premium = error
            .kind
            .strip_prefix("FLOOD_PREMIUM_WAIT_")
            .and_then(|x| x.parse::<i32>().ok());
        let slow = error
            .kind
            .strip_prefix("SLOWMODE_WAIT_")
            .and_then(|x| x.parse::<i32>().ok())
            .filter(|n| *n < 3);
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
                s.pub_events.push_back(Notice::NonPremiumDelayed(id));
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
            self.state
                .lock()
                .unwrap()
                .pub_events
                .push_back(Notice::LanguageReset);
        }
        if error.kind == "FROZEN_METHOD_INVALID" {
            self.state
                .lock()
                .unwrap()
                .pub_events
                .push_back(Notice::Frozen);
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
            let mut h = self.state.lock().unwrap().global_fail.take();
            if let Some(f) = &mut h {
                f(error, response);
            }
            self.state.lock().unwrap().global_fail = h;
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
        self.state.lock().unwrap().changes.push(tx);
        rx
    }
    fn emit(&self, event: InstanceEvent) {
        self.state.lock().unwrap().events.push_back(event);
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
        let owner = self.clone();
        let fail = self.clone();
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
            let owner = self.clone();
            let fail = self.clone();
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
            let owner = self.clone();
            let fail = self.clone();
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
        let owner = self.clone();
        let fail = self.clone();
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
        }
        let ids = s.sessions.keys().copied().collect::<Vec<_>>();
        drop(s);
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
