//! Main-thread ownership of the LocalSet session tasks and service timers.
use super::*;
use crate::{
    config_loader::ConfigLoader,
    connection_abstract::RuntimeClock,
    core_types::SystemRandom,
    session::{Session, SessionParameters},
    session_private::ClientInfo,
};
use std::{cell::Cell, rc::Rc};
use tg_base::timer::{SystemClock, ThreadAlarm, Timer, TimerHost};
#[derive(Clone, Copy)]
pub enum TransportPolicy {
    Automatic,
    TcpOnly,
    HttpOnly,
}
pub struct RuntimeInstance {
    instance: Instance,
    info: ClientInfo,
    policy: TransportPolicy,
    tasks: Vec<tokio::task::JoinHandle<Result<(), crate::connection_abstract::TransportError>>>,
    host: Rc<TimerHost>,
    expiry: Timer,
    expiry_due: Rc<Cell<bool>>,
    delayed: super::InstanceTimers,
    loader: Option<ConfigLoader>,
    dns: tg_base::timer::Timer,
    dns_client: reqwest::Client,
    dns_tx: mpsc::UnboundedSender<(crate::details::mtproto_domain_resolver::Request, Vec<u8>)>,
    dns_rx: mpsc::UnboundedReceiver<(crate::details::mtproto_domain_resolver::Request, Vec<u8>)>,
    dns_tasks: Vec<tokio::task::JoinHandle<()>>,
    special: Option<
        crate::special_config_request::SpecialConfigRunner<
            crate::special_config_request::ReqwestExecutor,
        >,
    >,
    time: Option<
        crate::special_config_request::SpecialConfigRunner<
            crate::special_config_request::ReqwestExecutor,
        >,
    >,
    special_generation: u64,
    web: Rc<crate::special_config_request::ReqwestExecutor>,
}
impl RuntimeInstance {
    pub fn new(mode: Mode, fields: Fields, info: ClientInfo, policy: TransportPolicy) -> Self {
        let instance = Instance::new(mode, fields);
        let host = TimerHost::new(Arc::new(SystemClock));
        host.set_alarm(Some(Rc::new(ThreadAlarm::new(
            Arc::new(SystemClock),
            || tg_runtime::on_main(|| {}),
        ))));
        let due = Rc::new(Cell::new(false));
        let fired = due.clone();
        let expiry = Timer::in_host(&host, Some(Rc::new(move || fired.set(true))));
        let (dns_tx, dns_rx) = mpsc::unbounded_channel();
        let dns = Timer::in_host(&host, Some(Rc::new(|| {})));
        instance.start();
        let delayed = super::InstanceTimers::new(instance.clone(), host.clone());
        Self {
            instance,
            info,
            policy,
            tasks: vec![],
            host,
            expiry,
            expiry_due: due,
            delayed,
            loader: None,
            dns,
            dns_tx,
            dns_rx,
            dns_tasks: vec![],
            dns_client: reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("DNS HTTPS"),
            special: None,
            time: None,
            special_generation: 0,
            web: Rc::new(crate::special_config_request::ReqwestExecutor::default()),
        }
    }
    fn new_special(
        &self,
        time: bool,
    ) -> crate::special_config_request::SpecialConfigRunner<
        crate::special_config_request::ReqwestExecutor,
    > {
        crate::special_config_request::SpecialConfigRunner::new(
            self.instance.is_test_mode(),
            time,
            self.instance.config_values().txt_domain_string,
            self.instance.user_phone(),
            &self.host,
            self.web.clone(),
            &mut SystemRandom,
        )
    }
    pub fn instance(&self) -> &Instance {
        &self.instance
    }
    pub fn process(&mut self) {
        self.host.process();
        self.instance.process(tg_runtime::now());
        self.delayed.process();
        if self.expiry_due.replace(false) {
            if let Some(wait) = self.instance.request_config_if_expired() {
                self.expiry.call_once(wait);
            }
        }
        for notice in self.instance.take_service_requests() {
            match notice {
                Notice::ConfigNeeded if self.loader.is_none() => {
                    let mut loader = ConfigLoader::new(
                        self.instance.clone(),
                        self.instance.user_phone(),
                        self.instance.proxy_settings().enabled,
                        &self.host,
                    );
                    loader.load();
                    self.loader = Some(loader);
                }
                Notice::HttpTimeNeeded if self.time.is_none() => {
                    self.time = Some(self.new_special(true));
                }
                _ => {}
            }
        }
        if let Some(loader) = &mut self.loader {
            loader.set_phone(self.instance.user_phone());
            loader.set_proxy_enabled(self.instance.proxy_settings().enabled);
        }
        let results = self
            .loader
            .as_mut()
            .map(|loader| loader.process(&mut SystemRandom))
            .unwrap_or_default();
        for (special, config) in results.into_iter().flatten() {
            self.instance.apply_config(&config, special);
            if !special {
                self.loader = None;
                if let Some(wait) = self.instance.request_config_if_expired() {
                    self.expiry.call_once(wait);
                }
            }
        }
        let generation = self
            .loader
            .as_ref()
            .filter(|l| l.special_active)
            .map(|l| l.special_generation);
        if let Some(generation) = generation {
            if generation != self.special_generation {
                self.special_generation = generation;
                self.special = Some(self.new_special(false));
            }
        } else {
            self.special = None;
            self.special_generation = 0;
        }
        let endpoints = self
            .special
            .as_mut()
            .map(|s| s.process(i64::from(tg_base::unixtime::http_now()), &mut SystemRandom))
            .unwrap_or_default();
        for endpoint in endpoints {
            if let Some(endpoint) = endpoint {
                if let Some(loader) = &mut self.loader {
                    loader.add_special_endpoint(endpoint);
                }
            } else {
                self.special = None;
                if let Some(loader) = &mut self.loader {
                    loader.special_active = false;
                }
            }
        }
        if let Some(time) = &mut self.time {
            time.process(i64::from(tg_base::unixtime::http_now()), &mut SystemRandom);
            if time.time_done() {
                self.time = None;
                self.instance.state.lock().unwrap().http_time_loading = false;
            }
        }
        while let Ok((request, bytes)) = self.dns_rx.try_recv() {
            self.instance
                .domain_request_finished(&request, &bytes, tg_runtime::now());
        }
        for request in self
            .instance
            .poll_domain_requests(tg_runtime::now(), &mut SystemRandom)
        {
            let tx = self.dns_tx.clone();
            let client = self.dns_client.clone();
            self.dns_tasks.push(tokio::task::spawn_local(async move {
                let bytes =
                    crate::details::mtproto_domain_resolver::perform_request(&client, &request)
                        .await
                        .unwrap_or_default();
                let _ = tx.send((request, bytes));
                tg_runtime::on_main(|| {});
            }));
        }
        if let Some(deadline) = self.instance.next_domain_deadline() {
            let wait = (deadline - tg_runtime::now()).max(0);
            if !self.dns.is_active() || self.dns.remaining_time() != wait {
                self.dns.call_once(wait);
            }
        } else {
            self.dns.cancel();
        }
        self.dns_tasks.retain(|task| !task.is_finished());
        for spawn in self.instance.take_session_spawns() {
            let clock = Arc::new(SystemClock);
            let host = TimerHost::new(clock.clone());
            let (mut session, handle) = Session::new(
                spawn.id,
                spawn.dc,
                self.instance.session_environment(),
                self.instance.store.clone(),
                SessionParameters {
                    clock: RuntimeClock,
                    random: SystemRandom,
                    host: host.clone(),
                    info: self.info.clone(),
                },
            );
            if !matches!(self.policy, TransportPolicy::Automatic) {
                session.set_protocol_policy(
                    matches!(self.policy, TransportPolicy::TcpOnly),
                    matches!(self.policy, TransportPolicy::HttpOnly),
                    false,
                );
            }
            let wake = handle.clone();
            host.set_alarm(Some(Rc::new(ThreadAlarm::new(clock, move || wake.wake()))));
            self.instance.attach_session(spawn.id, handle);
            self.tasks.push(tokio::task::spawn_local(session.run(
                super::network::NetworkTransport::with_instance(&self.instance),
            )));
        }
    }
}
impl Drop for RuntimeInstance {
    fn drop(&mut self) {
        self.loader = None;
        for task in self.dns_tasks.drain(..) {
            task.abort();
        }
        self.instance.shutdown();
        for task in self.tasks.drain(..) {
            task.abort();
        }
    }
}
