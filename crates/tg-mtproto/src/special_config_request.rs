//! Signed simple-config decoding. All integer fields retain TL little endian.
use crate::details::mtproto_rsa_public_key::{base64_decode, RsaPublicKey};
use aes::cipher::{BlockCipherDecrypt, KeyInit};
use tg_tl::Deserialize;
pub const PUBLIC_KEY:&str="-----BEGIN RSA PUBLIC KEY-----\nMIIBCgKCAQEAyr+18Rex2ohtVy8sroGPBwXD3DOoKCSpjDqYoXgCqB7ioln4eDCF\nfOBUlfXUEvM/fnKCpF46VkAftlb4VuPDeQSS/ZxZYEGqHaywlroVnXHIjgqoxiAd\n192xRGreuXIaUKmkwlM9JID9WS2jUsTpzQ91L8MEPLJ/4zrBwZua8W5fECwCCh2c\n9G5IzzBm+otMS/YKwmR1olzRCyEkyAEjXWqBI9Ftv5eG8m0VkBzOG655WIYdyV0H\nfDK/NWcvGqa0w/nriMD6mDjKOryamw0OP9QuYgMN0C9xMW9y8SmP4h92OAWodTYg\nY1hZCxdv6cs5UnW9+PWvS+WIbkh+GaWYxwIDAQAB\n-----END RSA PUBLIC KEY-----";
pub const REMOTE_PROJECT: &str = "peak-vista-421";
pub const FIRE_PROJECT: &str = "reserve-5a846";
pub const CONFIG_KEY: &str = "ipconfig";
pub const CONFIG_SUB_KEY: &str = "v3";
pub const API_KEY: &str = "AIzaSyC2-kAkpDsroixRXw-sTw-Wfqo4NxjMwwM";
pub const APP_ID: &str = "1:560508485281:web:4ee13a6af4e84d49e67ae0";
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Endpoint {
    pub dc_id: i32,
    pub ip: String,
    pub port: i32,
    pub secret: Vec<u8>,
}
pub fn check_phone_by_prefixes_rules(phone: &str, rules: &str) -> bool {
    let phone: String = phone.chars().filter(char::is_ascii_digit).collect();
    let mut result = false;
    for prefix in rules.split(',') {
        if prefix.is_empty() {
            result = true;
        } else if let Some(p) = prefix.strip_prefix('+') {
            if phone.starts_with(p) {
                result = true;
            }
        } else if let Some(p) = prefix.strip_prefix('-') {
            if phone.starts_with(p) {
                return false;
            }
        }
    }
    result
}
pub fn concatenate_dns_txt_fields(entries: &[String]) -> Vec<u8> {
    let mut entries: Vec<_> = entries.iter().enumerate().collect();
    entries.sort_by_key(|(i, s)| {
        (
            std::cmp::Reverse(s.encode_utf16().count()),
            std::cmp::Reverse(*i),
        )
    });
    entries
        .into_iter()
        .flat_map(|(_, s)| {
            s.chars()
                .map(|c| if u32::from(c) <= 255 { c as u8 } else { b'?' })
        })
        .collect()
}
pub fn decrypt_simple_config(
    bytes: &[u8],
    key: &RsaPublicKey,
) -> Result<tg_scheme::HelpConfigSimple, &'static str> {
    let clean: Vec<_> = bytes
        .iter()
        .copied()
        .filter(|b| b.is_ascii_alphanumeric() || b"+=/".contains(b))
        .collect();
    if clean.len() != 344 {
        return Err("Bad data size (base64)");
    }
    let decoded = base64_decode(std::str::from_utf8(&clean).unwrap()).ok_or("Bad base64")?;
    if decoded.len() != 256 {
        return Err("Bad data size (decoded)");
    }
    let decrypted = key.decrypt(&decoded).ok_or("RSA decrypt failed")?;
    decode_aes_payload(&decrypted)
}
pub fn decode_aes_payload(decrypted: &[u8]) -> Result<tg_scheme::HelpConfigSimple, &'static str> {
    if decrypted.len() != 256 {
        return Err("Bad decrypted size");
    }
    let cipher = aes::Aes256::new_from_slice(&decrypted[..32]).unwrap();
    let mut iv: [u8; 16] = decrypted[16..32].try_into().unwrap();
    let mut data = decrypted[32..].to_vec();
    for chunk in data.chunks_exact_mut(16) {
        let previous: [u8; 16] = chunk.try_into().unwrap();
        let mut block = previous.into();
        cipher.decrypt_block(&mut block);
        for i in 0..16 {
            chunk[i] = block[i] ^ iv[i];
        }
        iv = previous;
    }
    let hash = tg_base::openssl_help::sha256(&data[..208]);
    if hash[..16] != data[208..] {
        return Err("Bad digest.");
    }
    let words = tg_tl::bytes_to_primes(&data[..208]);
    let length = words[0];
    if length <= 0 || length > 208 || length & 3 != 0 {
        return Err("Bad length");
    }
    let mut reader = tg_tl::Reader::new(&words[1..]);
    let config = tg_scheme::HelpConfigSimple::read(&mut reader)
        .map_err(|_| "Could not read configSimple.")?;
    if reader.remaining() * 4 != 208 - length as usize {
        return Err("Bad read length");
    }
    Ok(config)
}
pub fn handle_response(
    config: &tg_scheme::HelpConfigSimple,
    phone: &str,
    now: i64,
) -> Result<Vec<Endpoint>, &'static str> {
    let data = config.c_help_config_simple();
    if now > i64::from(data.expires) {
        return Err("Bad date frame");
    }
    if data.rules.0.is_empty() {
        return Err("Empty simple config received.");
    }
    let mut endpoints = Vec::new();
    for rule in &data.rules.0 {
        let rule = rule.c_access_point_rule();
        if !check_phone_by_prefixes_rules(phone, &String::from_utf8_lossy(&rule.phone_prefix_rules))
        {
            continue;
        }
        for ip in &rule.ips.0 {
            let (ipv4, port, secret) = match ip {
                tg_scheme::IpPort::IpPort(d) => (d.ipv4, d.port, vec![]),
                tg_scheme::IpPort::IpPortSecret(d) => (d.ipv4, d.port, d.secret.clone()),
            };
            endpoints.push(Endpoint {
                dc_id: rule.dc_id,
                ip: std::net::Ipv4Addr::from(ipv4 as u32).to_string(),
                port,
                secret,
            });
        }
    }
    Ok(endpoints)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Service {
    Mozilla,
    Google,
    RemoteConfig,
    Realtime,
    FireStore,
}
#[derive(Clone, Debug)]
pub struct Attempt {
    pub service: Service,
    pub data: String,
    pub host: String,
}
#[derive(Clone, Debug)]
pub struct WebRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub payload: Vec<u8>,
}
pub fn api_domain(service: &str) -> String {
    format!("{service}.googleapis.com")
}
pub fn generate_instance_id(random: &mut impl crate::core_types::Random) -> String {
    let mut fid = [0; 17];
    random.fill(&mut fid);
    fid[0] = (fid[0] & 0xf0) | 7;
    crate::details::mtproto_rsa_public_key::base64_url_encode(&fid)[..22].into()
}
pub fn parse_remote_config_response(bytes: &[u8]) -> Vec<u8> {
    parse_json_field(bytes, &["entries", "ipconfigv3"])
}
pub fn parse_fire_store_response(bytes: &[u8]) -> Vec<u8> {
    parse_json_field(bytes, &["fields", "data", "stringValue"])
}
fn parse_json_field(bytes: &[u8], path: &[&str]) -> Vec<u8> {
    let Ok(value) = tg_base::json::from_slice::<tg_base::json::Value>(bytes) else {
        return vec![];
    };
    let mut current = &value;
    for name in path {
        let Some(next) = current.get(*name) else {
            return vec![];
        };
        current = next;
    }
    current
        .as_str()
        .unwrap_or("")
        .chars()
        .map(|c| if c as u32 <= 255 { c as u8 } else { b'?' })
        .collect()
}
pub fn parse_realtime_response(bytes: &[u8]) -> Vec<u8> {
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        bytes[1..bytes.len() - 1].to_vec()
    } else {
        vec![]
    }
}
pub fn request_for_attempt(
    attempt: &Attempt,
    domain: &str,
    instance_id: &str,
    random: &mut impl crate::core_types::Random,
) -> WebRequest {
    use crate::details::mtproto_domain_resolver::{generate_dns_random_padding, DNS_USER_AGENT};
    let mut headers = vec![("User-Agent".into(), DNS_USER_AGENT.into())];
    let mut payload = vec![];
    let url = match attempt.service {
        Service::Mozilla => {
            headers.push(("accept".into(), "application/dns-json".into()));
            format!(
                "https://{}/dns-query?name={domain}&type=16&random_padding={}",
                attempt.data,
                generate_dns_random_padding(random)
            )
        }
        Service::Google => {
            if !attempt.host.is_empty() {
                headers.push(("Host".into(), format!("{}.google.com", attempt.host)));
            }
            format!(
                "https://{}/resolve?name={domain}&type=ANY&random_padding={}",
                attempt.data,
                generate_dns_random_padding(random)
            )
        }
        Service::RemoteConfig => {
            payload = format!("{{\"app_id\":\"{APP_ID}\",\"app_instance_id\":\"{instance_id}\"}}")
                .into_bytes();
            headers.push(("Content-Type".into(), "application/json".into()));
            format!(
                "https://{}/v1/projects/{REMOTE_PROJECT}/namespaces/firebase:fetch?key={API_KEY}",
                api_domain(&attempt.data)
            )
        }
        Service::Realtime => format!(
            "https://{FIRE_PROJECT}.{}/{CONFIG_KEY}{CONFIG_SUB_KEY}.json",
            attempt.data
        ),
        Service::FireStore => {
            let host = if attempt.host.is_empty() {
                api_domain(&attempt.data)
            } else {
                headers.push(("Host".into(), api_domain(&attempt.host)));
                attempt.data.clone()
            };
            format!("https://{host}/v1/projects/{FIRE_PROJECT}/databases/(default)/documents/{CONFIG_KEY}/{CONFIG_SUB_KEY}")
        }
    };
    WebRequest {
        url,
        headers,
        payload,
    }
}
pub fn parse_service_response(service: Service, bytes: &[u8]) -> Vec<u8> {
    match service {
        Service::Google | Service::Mozilla => {
            let entries =
                crate::details::mtproto_domain_resolver::parse_dns_response(bytes, Some(16));
            concatenate_dns_txt_fields(&entries.into_iter().map(|e| e.data).collect::<Vec<_>>())
        }
        Service::RemoteConfig => parse_remote_config_response(bytes),
        Service::Realtime => parse_realtime_response(bytes),
        Service::FireStore => parse_fire_store_response(bytes),
    }
}
pub struct SpecialConfigRequest {
    attempts: std::collections::VecDeque<Attempt>,
    next: std::rc::Rc<std::cell::Cell<bool>>,
    timer: tg_base::timer::Timer,
}
impl SpecialConfigRequest {
    pub fn new(
        test: bool,
        time_only: bool,
        host: &std::rc::Rc<tg_base::timer::TimerHost>,
        random: &mut impl crate::core_types::Random,
    ) -> Self {
        let mut dns = vec![
            Attempt {
                service: Service::Google,
                data: "dns.google.com".into(),
                host: String::new(),
            },
            Attempt {
                service: Service::Mozilla,
                data: "mozilla.cloudflare-dns.com".into(),
                host: String::new(),
            },
        ];
        if random.u32() & 1 != 0 {
            dns.swap(0, 1);
        }
        let mut fire = vec![Attempt {
            service: Service::FireStore,
            data: "firestore".into(),
            host: String::new(),
        }];
        for domain in crate::details::mtproto_domain_resolver::DNS_DOMAINS {
            fire.push(Attempt {
                service: Service::FireStore,
                data: domain.into(),
                host: "firestore".into(),
            });
        }
        for i in (1..fire.len()).rev() {
            fire.swap(i, random.u32() as usize % (i + 1));
        }
        if !test && !time_only {
            dns.extend(fire);
        }
        let next = std::rc::Rc::new(std::cell::Cell::new(true));
        let due = next.clone();
        let timer =
            tg_base::timer::Timer::in_host(host, Some(std::rc::Rc::new(move || due.set(true))));
        Self {
            attempts: dns.into(),
            next,
            timer,
        }
    }
    pub fn send_next_request(&mut self) -> Option<Attempt> {
        if !self.next.replace(false) {
            return None;
        }
        let result = self.attempts.pop_front();
        if !self.attempts.is_empty() {
            self.timer.call_once(800);
        }
        result
    }
}
