//! DoH attempts, 800 ms stagger, TTL cache and response parsing over explicit time.
use crate::{connection_abstract::TransportError, core_types::Random};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
pub const DNS_DOMAINS: [&str; 4] = ["google.com", "www.google.com", "google.ru", "www.google.ru"];
pub const DNS_USER_AGENT:&str="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36";
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DnsEntry {
    pub data: String,
    pub ttl: i64,
}
pub fn parse_dns_response(bytes: &[u8], type_restriction: Option<i32>) -> Vec<DnsEntry> {
    let Ok(value) = tg_base::json::from_slice::<tg_base::json::Value>(bytes) else {
        return Vec::new();
    };
    let Some(answers) = value.get("Answer").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    answers
        .iter()
        .filter_map(|v| {
            if let Some(kind) = type_restriction {
                if v.get("type")?.as_f64()?.round() as i32 != kind {
                    return None;
                }
            }
            Some(DnsEntry {
                data: v.get("data")?.as_str()?.into(),
                ttl: v.get("TTL").and_then(|v| v.as_f64()).unwrap_or(0.0).round() as i64,
            })
        })
        .collect()
}
pub fn generate_dns_random_padding(random: &mut impl Random) -> String {
    const ABC: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let size = loop {
        let mut b = [0];
        random.fill(&mut b);
        let n = 1 + b[0] / 2;
        if n >= 13 {
            break n as usize;
        }
    };
    (0..size)
        .map(|_| {
            let mut b = [0];
            random.fill(&mut b);
            ABC[b[0] as usize % 62] as char
        })
        .collect()
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct AttemptKey {
    pub domain: String,
    pub ipv6: bool,
}
#[derive(Clone, Debug)]
pub struct Request {
    pub id: u64,
    pub key: AttemptKey,
    pub url: String,
    pub host: Option<String>,
    pub mozilla: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resolved {
    pub domain: String,
    pub ips: Vec<String>,
    pub expire_at: i64,
}
#[derive(Clone)]
struct CacheEntry {
    ips: Vec<String>,
    expire_at: i64,
}
#[derive(Clone)]
struct Attempt {
    domain: String,
    host: Option<String>,
    mozilla: bool,
}
struct Attempts {
    list: Vec<Attempt>,
    deadline: Option<i64>,
    pending: BTreeSet<u64>,
}
#[derive(Default)]
pub struct DomainResolver {
    attempts: BTreeMap<AttemptKey, Attempts>,
    cache: BTreeMap<AttemptKey, CacheEntry>,
    next_id: u64,
    pub resolved: VecDeque<Resolved>,
}
fn shuffle<T>(values: &mut [T], random: &mut impl Random) {
    for i in (1..values.len()).rev() {
        let count = (i + 1) as u32;
        let max = u32::MAX / count * count;
        let index = loop {
            let value = random.u32();
            if value < max {
                break (value % count) as usize;
            }
        };
        values.swap(i, index);
    }
}
impl DomainResolver {
    pub fn resolve(&mut self, domain: &str, now: i64, random: &mut impl Random) -> Vec<Request> {
        let mut requests = Vec::new();
        for ipv6 in [false, true] {
            let key = AttemptKey {
                domain: domain.into(),
                ipv6,
            };
            if self.attempts.contains_key(&key) {
                continue;
            }
            if self.cache.get(&key).is_some_and(|c| c.expire_at > now) {
                self.push_result(domain, now);
                continue;
            }
            let mut domains = DNS_DOMAINS.to_vec();
            shuffle(&mut domains, random);
            let mut list = vec![
                Attempt {
                    domain: "dns.google.com".into(),
                    host: None,
                    mozilla: false,
                },
                Attempt {
                    domain: domains.pop().unwrap().into(),
                    host: Some("dns.google.com".into()),
                    mozilla: false,
                },
                Attempt {
                    domain: "mozilla.cloudflare-dns.com".into(),
                    host: None,
                    mozilla: true,
                },
            ];
            while let Some(domain) = domains.pop() {
                list.push(Attempt {
                    domain: domain.into(),
                    host: Some("dns.google.com".into()),
                    mozilla: false,
                });
            }
            shuffle(&mut list[..2], random);
            list.reverse();
            self.attempts.insert(
                key.clone(),
                Attempts {
                    list,
                    deadline: None,
                    pending: BTreeSet::new(),
                },
            );
            if let Some(request) = self.next_request(&key, now, random) {
                requests.push(request);
            }
        }
        requests
    }
    fn next_request(
        &mut self,
        key: &AttemptKey,
        now: i64,
        random: &mut impl Random,
    ) -> Option<Request> {
        let attempts = self.attempts.get_mut(key)?;
        let attempt = attempts.list.pop()?;
        attempts.deadline = (!attempts.list.is_empty()).then_some(now + 800);
        self.next_id += 1;
        attempts.pending.insert(self.next_id);
        let mut url = reqwest::Url::parse(&format!(
            "https://{}/{}",
            attempt.domain,
            if attempt.mozilla {
                "dns-query"
            } else {
                "resolve"
            }
        ))
        .ok()?;
        url.query_pairs_mut()
            .append_pair("name", &key.domain)
            .append_pair("type", if key.ipv6 { "28" } else { "1" })
            .append_pair("random_padding", &generate_dns_random_padding(random));
        Some(Request {
            id: self.next_id,
            key: key.clone(),
            url: url.into(),
            host: attempt.host,
            mozilla: attempt.mozilla,
        })
    }
    pub fn poll(&mut self, now: i64, random: &mut impl Random) -> Vec<Request> {
        let keys: Vec<_> = self
            .attempts
            .iter()
            .filter(|(_, a)| a.deadline.is_some_and(|t| t <= now))
            .map(|(k, _)| k.clone())
            .collect();
        keys.iter()
            .filter_map(|key| self.next_request(key, now, random))
            .collect()
    }
    pub fn next_deadline(&self) -> Option<i64> {
        self.attempts.values().filter_map(|a| a.deadline).min()
    }
    pub fn request_finished(&mut self, request: &Request, bytes: &[u8], now: i64) {
        let Some(attempts) = self.attempts.get_mut(&request.key) else {
            return;
        };
        if !attempts.pending.remove(&request.id) {
            return;
        }
        let response = parse_dns_response(bytes, None);
        if response.is_empty() {
            return;
        }
        self.attempts.remove(&request.key);
        let ttl = response
            .iter()
            .map(|e| e.ttl.saturating_mul(1000).max(10000))
            .min()
            .unwrap()
            .min(300000);
        self.cache.insert(
            request.key.clone(),
            CacheEntry {
                ips: response.into_iter().map(|e| e.data).collect(),
                expire_at: now + ttl,
            },
        );
        self.push_result(&request.key.domain, now);
    }
    fn push_result(&mut self, domain: &str, now: i64) {
        let key = AttemptKey {
            domain: domain.into(),
            ipv6: false,
        };
        let Some(entry) = self.cache.get(&key).filter(|c| c.expire_at > now) else {
            return;
        };
        let mut entry = entry.clone();
        if let Some(ipv6) = self
            .cache
            .get(&AttemptKey {
                domain: domain.into(),
                ipv6: true,
            })
            .filter(|c| c.expire_at > now)
        {
            entry.ips.extend(ipv6.ips.clone());
            entry.expire_at = entry.expire_at.min(ipv6.expire_at);
        }
        self.resolved.push_back(Resolved {
            domain: domain.into(),
            ips: entry.ips,
            expire_at: entry.expire_at,
        });
    }
    pub fn is_request_active(&self, request: &Request) -> bool {
        self.attempts
            .get(&request.key)
            .is_some_and(|a| a.pending.contains(&request.id))
    }
}
pub async fn perform_request(
    client: &reqwest::Client,
    request: &Request,
) -> Result<Vec<u8>, TransportError> {
    let mut builder = client
        .get(&request.url)
        .header(reqwest::header::USER_AGENT, DNS_USER_AGENT);
    if request.mozilla {
        builder = builder.header(reqwest::header::ACCEPT, "application/dns-json");
    }
    if let Some(host) = &request.host {
        builder = builder.header(reqwest::header::HOST, host);
    }
    Ok(builder.send().await?.bytes().await?.to_vec())
}
/// Cancellation owner analogous to ServiceWebRequest; dropping aborts a pending task.
pub struct ServiceWebRequest<T>(Option<tokio::task::JoinHandle<T>>);
impl<T> ServiceWebRequest<T> {
    pub fn new(task: tokio::task::JoinHandle<T>) -> Self {
        Self(Some(task))
    }
    pub fn destroy(&mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
        }
    }
}
impl<T> Drop for ServiceWebRequest<T> {
    fn drop(&mut self) {
        self.destroy();
    }
}
