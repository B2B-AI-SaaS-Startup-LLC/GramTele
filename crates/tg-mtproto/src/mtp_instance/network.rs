//! Tokio executor for Session's independent incoming and outgoing queues.
use crate::{
    connection_abstract::{RuntimeClock, TransportError},
    connection_http::HttpConnection,
    connection_tcp::{Protocol, TcpConnection},
    core_types::SystemRandom,
    session::{SessionTransport, TransportEvent},
    session_private::{ConnectionSpec, TransportAction},
};
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::mpsc;
pub struct NetworkTransport {
    owner: Option<super::WeakInstance>,
    tls_ids: std::collections::BTreeSet<u64>,
    tx: mpsc::UnboundedSender<TransportEvent>,
    rx: mpsc::UnboundedReceiver<TransportEvent>,
    connections: BTreeMap<
        u64,
        (
            mpsc::UnboundedSender<crate::core_types::Buffer>,
            tokio::task::JoinHandle<()>,
        ),
    >,
}
impl Default for NetworkTransport {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            owner: None,
            tls_ids: Default::default(),
            tx,
            rx,
            connections: BTreeMap::new(),
        }
    }
}
impl NetworkTransport {
    async fn connection_direct(
        spec: ConnectionSpec,
        outgoing: &mut mpsc::UnboundedReceiver<crate::core_types::Buffer>,
        tx: mpsc::UnboundedSender<TransportEvent>,
        connected: std::rc::Rc<std::cell::Cell<bool>>,
        ready: Arc<tokio::sync::Notify>,
        full_timeout: i64,
    ) -> Result<(), TransportError> {
        let id = spec.id;
        if spec.http {
            let connection = Arc::new(HttpConnection::new(&spec.endpoint.ip, spec.proxy.clone()));
            connection
                .probe(
                    tg_base::unixtime::mtproto_msg_id(),
                    &RuntimeClock,
                    &mut SystemRandom,
                )
                .await?;
            connected.set(true);
            ready.notify_one();
            let _ = tx.send(TransportEvent::ProbeConnected {
                id,
                tag: "HTTP".into(),
                full_timeout,
                ping_time: connection.state.lock().unwrap().ping_time,
            });
            let mut tasks = TaskBag(Vec::new());
            while let Some(packet) = outgoing.recv().await {
                tasks.0.retain(|task| !task.is_finished());
                let connection = connection.clone();
                let tx = tx.clone();
                tasks.0.push(tokio::task::spawn_local(async move {
                    match connection.send_data(packet).await {
                        Ok(words) if words.is_empty() => {}
                        Ok(words) => {
                            let _ = tx.send(TransportEvent::Packet {
                                id,
                                words,
                                need_http_wait: connection.need_http_wait(),
                            });
                        }
                        Err(error) => send_error(&tx, id, error),
                    }
                }));
            }
        } else {
            let fake_tls = spec.endpoint.secret.first() == Some(&0xee);
            let stream = crate::details::mtproto_tcp_socket::connect_to_host(
                &spec.endpoint.ip,
                spec.endpoint.port as u16,
                &spec.proxy,
                spec.for_files,
            )
            .await
            .inspect_err(|_| {
                if fake_tls {
                    let _ = tx.send(TransportEvent::SyncTimeRequest(id));
                }
            })?;
            let stream: Box<dyn crate::details::mtproto_abstract_socket::ByteStream + Send> =
                if fake_tls {
                    Box::new(
                        crate::details::mtproto_tls_socket::TlsSocket::connect(
                            stream,
                            &spec.endpoint.secret,
                            tg_base::unixtime::now(),
                            &mut crate::details::mtproto_tls_socket::SystemHelloEntropy,
                        )
                        .await
                        .inspect_err(|_| {
                            let _ = tx.send(TransportEvent::SyncTimeRequest(id));
                        })?,
                    )
                } else {
                    Box::new(stream)
                };
            let mut connection = TcpConnection::new_with_nonce_filter(
                stream,
                Protocol::from_secret(&spec.endpoint.secret)?,
                spec.protocol_dc_id,
                SystemRandom,
                move |nonce: &[u8; 64]| {
                    fake_tls || crate::connection_tcp::is_good_start_nonce(nonce)
                },
            )
            .await?;
            connection
                .probe(tg_base::unixtime::mtproto_msg_id(), &RuntimeClock)
                .await?;
            connected.set(true);
            ready.notify_one();
            let _ = tx.send(TransportEvent::ProbeConnected {
                id,
                tag: "TCP".into(),
                full_timeout,
                ping_time: connection.state.ping_time,
            });
            let (mut reader, mut writer) = connection.into_split();
            let read = async {
                loop {
                    let words = reader.receive().await?;
                    if tx
                        .send(TransportEvent::Packet {
                            id,
                            words,
                            need_http_wait: false,
                        })
                        .is_err()
                    {
                        return Ok::<(), TransportError>(());
                    }
                }
            };
            let write = async {
                while let Some(packet) = outgoing.recv().await {
                    writer.send_data(packet).await?;
                }
                Ok::<(), TransportError>(())
            };
            tokio::select! {result=read=>result?,result=write=>result?};
        }
        Ok(())
    }
}
fn send_error(tx: &mpsc::UnboundedSender<TransportEvent>, id: u64, error: TransportError) {
    let event = match error {
        TransportError::Server(code) => TransportEvent::Error(id, code),
        _ => TransportEvent::Disconnected(id),
    };
    let _ = tx.send(event);
}
impl SessionTransport for NetworkTransport {
    fn submit(&mut self, action: TransportAction) -> Result<(), TransportError> {
        match action {
            TransportAction::Connect(spec) => {
                let id = spec.id;
                if !spec.http && spec.endpoint.secret.first() == Some(&0xee) {
                    self.tls_ids.insert(id);
                }
                let (tx, rx) = mpsc::unbounded_channel();
                let events = self.tx.clone();
                let owner = self.owner.clone();
                let task = tokio::task::spawn_local(async move {
                    if let Err(error) = Self::connection(spec, rx, events.clone(), owner).await {
                        send_error(&events, id, error);
                    }
                });
                self.connections.insert(id, (tx, task));
            }
            TransportAction::Send(id, packet) => {
                if let Some((tx, _)) = self.connections.get(&id) {
                    let _ = tx.send(packet);
                }
            }
            TransportAction::Close(id) => {
                self.tls_ids.remove(&id);
                if let Some((_, task)) = self.connections.remove(&id) {
                    task.abort();
                }
            }
            TransportAction::TimedOut(id) => {
                if self.tls_ids.contains(&id) {
                    let _ = self.tx.send(TransportEvent::SyncTimeRequest(id));
                }
            }
        }
        Ok(())
    }
    async fn next_event(&mut self) -> Result<TransportEvent, TransportError> {
        self.rx
            .recv()
            .await
            .ok_or(TransportError::Protocol("network executor closed"))
    }
}
impl Drop for NetworkTransport {
    fn drop(&mut self) {
        for (_, (_, task)) in std::mem::take(&mut self.connections) {
            task.abort();
        }
    }
}

impl NetworkTransport {
    pub fn with_instance(instance: &super::Instance) -> Self {
        let mut result = Self::default();
        result.owner = Some(instance.downgrade());
        result
    }
    async fn connection(
        spec: ConnectionSpec,
        mut outgoing: mpsc::UnboundedReceiver<crate::core_types::Buffer>,
        tx: mpsc::UnboundedSender<TransportEvent>,
        owner: Option<super::WeakInstance>,
    ) -> Result<(), TransportError> {
        let ready = Arc::new(tokio::sync::Notify::new());
        if !spec.proxy.try_custom_resolve() || owner.is_none() {
            return Self::connection_direct(
                spec,
                &mut outgoing,
                tx,
                std::rc::Rc::new(std::cell::Cell::new(false)),
                ready,
                8000,
            )
            .await;
        }
        let owner = owner.unwrap();
        let instance = owner
            .upgrade()
            .ok_or(TransportError::Protocol("Instance destroyed"))?;
        let mut updates = instance.subscribe_proxy_domains();
        drop(instance);
        let mut resolving = crate::connection_resolving::ResolvingConnection::new(
            spec.proxy.clone(),
            tg_runtime::now(),
        );
        let clock = Arc::new(tg_base::timer::SystemClock);
        let host = tg_base::timer::TimerHost::new(clock.clone());
        let wake = Arc::new(tokio::sync::Notify::new());
        let alarm = wake.clone();
        host.set_alarm(Some(std::rc::Rc::new(tg_base::timer::ThreadAlarm::new(
            clock,
            move || alarm.notify_one(),
        ))));
        let due = std::rc::Rc::new(std::cell::Cell::new(false));
        let expired = due.clone();
        let timer = tg_base::timer::Timer::in_host(
            &host,
            Some(std::rc::Rc::new(move || expired.set(true))),
        );
        let mut next = resolving_actions(&mut resolving, &owner)?;
        loop {
            let proxy = loop {
                if let Some(proxy) = next.take() {
                    break proxy;
                }
                let update = updates
                    .recv()
                    .await
                    .ok_or(TransportError::Protocol("proxy resolver closed"))?;
                resolving.domain_resolved(
                    &update.domain,
                    &update.ips,
                    update.expire_at,
                    tg_runtime::now(),
                );
                let _ = tx.send(TransportEvent::ConnectTimeout {
                    id: spec.id,
                    full_timeout: resolving.full_connect_timeout(),
                });
                next = resolving_actions(&mut resolving, &owner)?;
            };
            let mut direct = spec.clone();
            direct.proxy = proxy;
            if direct.proxy.kind == crate::mtproto_proxy_data::Type::Mtproto {
                direct.endpoint.ip = direct.proxy.host.clone();
            }
            let connected = std::rc::Rc::new(std::cell::Cell::new(false));
            let mut ready_handled = false;
            let ready = Arc::new(tokio::sync::Notify::new());
            let connect = Self::connection_direct(
                direct,
                &mut outgoing,
                tx.clone(),
                connected.clone(),
                ready.clone(),
                resolving.full_connect_timeout(),
            );
            tokio::pin!(connect);
            loop {
                if let Some(at) = resolving.deadline {
                    timer.call_once((at - tg_runtime::now()).max(0));
                } else {
                    timer.cancel();
                }
                tokio::select! {
                    result=&mut connect=>{match result{Err(TransportError::Server(code))=>resolving.handle_error(code,tg_runtime::now()),_=>resolving.handle_disconnected(tg_runtime::now())};next=resolving_actions(&mut resolving,&owner)?;break;},
                    _=ready.notified(),if !ready_handled=>{ready_handled=true;resolving.handle_connected();next=resolving_actions(&mut resolving,&owner)?;if next.is_some(){break;}},
                    update=updates.recv()=>{let update=update.ok_or(TransportError::Protocol("proxy resolver closed"))?;resolving.domain_resolved(&update.domain,&update.ips,update.expire_at,tg_runtime::now());let _=tx.send(TransportEvent::ConnectTimeout{id:spec.id,full_timeout:resolving.full_connect_timeout()});next=resolving_actions(&mut resolving,&owner)?;if next.is_some(){break;}},
                    _=wake.notified()=>{host.process();if due.replace(false){resolving.poll(tg_runtime::now());next=resolving_actions(&mut resolving,&owner)?;if next.is_some(){break;}}}
                }
            }
        }
    }
}

struct TaskBag(Vec<tokio::task::JoinHandle<()>>);
impl Drop for TaskBag {
    fn drop(&mut self) {
        for task in self.0.drain(..) {
            task.abort();
        }
    }
}

fn resolving_actions(
    resolving: &mut crate::connection_resolving::ResolvingConnection,
    owner: &super::WeakInstance,
) -> Result<Option<crate::mtproto_proxy_data::ProxyData>, TransportError> {
    use crate::{connection_abstract::Event, connection_resolving::Action};
    let mut next = None;
    for action in resolving.actions.drain(..) {
        match action {
            Action::ResolveProxyDomain { host } => {
                if let Some(instance) = owner.upgrade() {
                    instance.resolve_proxy_domain(&host, tg_runtime::now(), &mut SystemRandom);
                }
            }
            Action::ReplaceChild(proxy) => next = Some(proxy),
            Action::SetGoodProxyDomain { host, ip } => {
                if let Some(instance) = owner.upgrade() {
                    instance.set_good_proxy_domain(&host, &ip);
                }
            }
            Action::Event(Event::Error(code)) => return Err(TransportError::Server(code)),
            Action::Event(Event::Disconnected) => {
                return Err(TransportError::Io(std::io::Error::other(
                    "proxy disconnected",
                )))
            }
            Action::Event(_) => {}
        }
    }
    Ok(next)
}
