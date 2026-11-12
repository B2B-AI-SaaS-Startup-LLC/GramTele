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
            tx,
            rx,
            connections: BTreeMap::new(),
        }
    }
}
impl NetworkTransport {
    async fn connection(
        spec: ConnectionSpec,
        mut outgoing: mpsc::UnboundedReceiver<crate::core_types::Buffer>,
        tx: mpsc::UnboundedSender<TransportEvent>,
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
            let _ = tx.send(TransportEvent::Connected {
                id,
                tag: "HTTP".into(),
                full_timeout: 8000,
            });
            let mut tasks = Vec::new();
            while let Some(packet) = outgoing.recv().await {
                let connection = connection.clone();
                let tx = tx.clone();
                tasks.push(tokio::task::spawn_local(async move {
                    match connection.send_data(packet).await {
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
            for task in tasks {
                task.abort();
            }
        } else {
            let stream = crate::details::mtproto_tcp_socket::connect_to_host(
                &spec.endpoint.ip,
                spec.endpoint.port as u16,
                &spec.proxy,
                false,
            )
            .await?;
            let mut connection = TcpConnection::new(
                stream,
                Protocol::from_secret(&spec.endpoint.secret)?,
                spec.protocol_dc_id as i16,
                SystemRandom,
            )
            .await?;
            connection
                .probe(tg_base::unixtime::mtproto_msg_id(), &RuntimeClock)
                .await?;
            let _ = tx.send(TransportEvent::Connected {
                id,
                tag: "TCP".into(),
                full_timeout: 8000,
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
                let (tx, rx) = mpsc::unbounded_channel();
                let events = self.tx.clone();
                let task = tokio::task::spawn_local(async move {
                    if let Err(error) = Self::connection(spec, rx, events.clone()).await {
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
                if let Some((_, task)) = self.connections.remove(&id) {
                    task.abort();
                }
            }
            TransportAction::TimedOut(_) => {}
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
