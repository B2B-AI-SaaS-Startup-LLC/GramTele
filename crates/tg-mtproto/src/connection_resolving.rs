//! Proxy IP fallback state; driver owns the concrete child stream and wake source.
use crate::{
    connection_abstract::Event,
    mtproto_proxy_data::{to_direct_ip_proxy, ProxyData},
};
use std::collections::VecDeque;
#[derive(Clone, Debug)]
pub enum Action {
    ResolveProxyDomain { host: String },
    ReplaceChild(ProxyData),
    SetGoodProxyDomain { host: String, ip: String },
    Event(Event),
}
pub struct ResolvingConnection {
    pub proxy: ProxyData,
    pub connected: bool,
    pub ip_index: isize,
    pub deadline: Option<i64>,
    alive: bool,
    pub actions: VecDeque<Action>,
}
impl ResolvingConnection {
    pub fn new(proxy: ProxyData, now: i64) -> Self {
        let mut result = Self {
            proxy,
            connected: false,
            ip_index: -1,
            deadline: None,
            alive: true,
            actions: VecDeque::new(),
        };
        if result.proxy.resolved_expire_at < now {
            result.actions.push_back(Action::ResolveProxyDomain {
                host: result.proxy.host.clone(),
            });
        }
        if !result.proxy.resolved_ips.is_empty() {
            result.refresh_child(now);
        }
        result
    }
    pub fn full_connect_timeout(&self) -> i64 {
        4000 * self.proxy.resolved_ips.len().max(1) as i64
    }
    fn refresh_child(&mut self, now: i64) -> bool {
        if !self.alive {
            return true;
        }
        self.ip_index += 1;
        if self.ip_index >= self.proxy.resolved_ips.len() as isize {
            return false;
        }
        self.actions
            .push_back(Action::ReplaceChild(to_direct_ip_proxy(
                &self.proxy,
                self.ip_index as usize,
            )));
        self.deadline = Some(now + 4000);
        true
    }
    fn emit_error(&mut self, code: i32) {
        self.ip_index = -1;
        self.alive = false;
        self.actions.push_back(Action::Event(Event::Error(code)));
    }
    pub fn handle_error(&mut self, code: i32, now: i64) {
        if self.connected || !self.proxy.resolved_ips.is_empty() && !self.refresh_child(now) {
            self.emit_error(code);
        }
    }
    pub fn handle_disconnected(&mut self, now: i64) {
        if self.connected {
            self.actions.push_back(Action::Event(Event::Disconnected));
        } else {
            self.handle_error(-499, now);
        }
    }
    pub fn poll(&mut self, now: i64) {
        if self.deadline.is_some_and(|t| t <= now) {
            self.deadline = None;
            self.handle_error(-499, now);
        }
    }
    pub fn handle_connected(&mut self) {
        self.connected = true;
        self.deadline = None;
        if self.ip_index >= 0 {
            self.actions.push_back(Action::SetGoodProxyDomain {
                host: self.proxy.host.clone(),
                ip: self.proxy.resolved_ips[self.ip_index as usize].clone(),
            });
        }
        self.actions.push_back(Action::Event(Event::Connected));
    }
    pub fn domain_resolved(&mut self, host: &str, ips: &[String], expire_at: i64, now: i64) {
        if host != self.proxy.host || !self.alive {
            return;
        }
        self.proxy.resolved_expire_at = expire_at;
        for (index, ip) in ips.iter().enumerate() {
            if index >= self.proxy.resolved_ips.len() {
                self.proxy.resolved_ips.push(ip.clone());
            } else if self.proxy.resolved_ips[index] != *ip {
                self.proxy.resolved_ips[index] = ip.clone();
                if self.ip_index >= index as isize {
                    self.ip_index = index as isize - 1;
                    self.refresh_child(now);
                }
            }
        }
        if ips.len() < self.proxy.resolved_ips.len() {
            self.proxy.resolved_ips.truncate(ips.len());
            if self.ip_index >= ips.len() as isize {
                self.emit_error(-499);
            }
        }
        if self.ip_index < 0 {
            self.refresh_child(now);
        }
    }
    pub fn disconnect_from_server(&mut self) {
        self.deadline = None;
        self.connected = false;
        self.alive = false;
    }
}
