//! Probe candidates share the same transport executor as sessions.
use crate::{
    connection_abstract::TransportError,
    mtp_instance::Instance,
    mtproto_dc_options::DcType,
    mtproto_proxy_data::{ProxyData, Type},
    session::{SessionEnvironment, SessionTransport, TransportEvent},
    session_private::{ConnectionSpec, TransportAction},
};
use std::collections::BTreeMap;
#[derive(Default)]
pub struct ProxyCheckers {
    checks: BTreeMap<u64, i64>,
    next: u64,
}
impl ProxyCheckers {
    pub fn has_proxy_checkers(&self) -> bool {
        !self.checks.is_empty()
    }
    pub fn reset_proxy_checkers(
        &mut self,
        transport: &mut impl SessionTransport,
    ) -> Result<(), TransportError> {
        for (id, _) in std::mem::take(&mut self.checks) {
            transport.submit(TransportAction::Close(id))?;
        }
        Ok(())
    }
    pub fn drop_proxy_checker(
        &mut self,
        id: u64,
        transport: &mut impl SessionTransport,
    ) -> Result<(), TransportError> {
        if self.checks.remove(&id).is_some() {
            transport.submit(TransportAction::Close(id))?;
        }
        Ok(())
    }
    pub fn start_proxy_check(
        &mut self,
        instance: &Instance,
        proxy: &ProxyData,
        try_ipv6: bool,
        now: i64,
        transport: &mut impl SessionTransport,
    ) -> Result<(), TransportError> {
        self.reset_proxy_checkers(transport)?;
        if proxy.kind == Type::Web {
            return Ok(());
        }
        let http = proxy.kind == Type::Http;
        let dc = instance.main_dc_id();
        let endpoints = if proxy.kind == Type::Mtproto {
            vec![crate::mtproto_dc_options::Endpoint {
                id: dc,
                flags: 0,
                ip: proxy.host.clone(),
                port: proxy.port as i32,
                secret: proxy.secret_from_mtproto_password(),
            }]
        } else {
            let options = instance.dc_options().lookup(dc, DcType::Regular, true);
            (0..if try_ipv6 { 2 } else { 1 })
                .filter_map(|ip| options[ip][usize::from(http)].first().cloned())
                .collect()
        };
        for endpoint in endpoints {
            self.next += 1;
            let id = self.next;
            self.checks.insert(id, now);
            transport.submit(TransportAction::Connect(ConnectionSpec {
                id,
                endpoint,
                http,
                priority: 0,
                protocol_dc_id: (dc + if instance.is_test_mode() { 10000 } else { 0 }) as i16,
                for_files: false,
                proxy: proxy.clone(),
            }))?;
        }
        Ok(())
    }
    pub fn process_event(
        &mut self,
        event: &TransportEvent,
        _now: i64,
    ) -> Option<(u64, Result<i32, ()>)> {
        let (id, result) = match event {
            TransportEvent::ProbeConnected { id, ping_time, .. } => (*id, Ok(*ping_time as i32)),
            TransportEvent::Disconnected(id) | TransportEvent::Error(id, _) => (*id, Err(())),
            _ => return None,
        };
        self.checks.contains_key(&id).then_some((id, result))
    }
}
