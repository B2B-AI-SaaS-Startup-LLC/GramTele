//! Proxy descriptors, secret/link validation, and the fork's WEB bridge capability.
use crate::details::{
    mtproto_rsa_public_key::{base64_decode, base64_url_encode},
    qt_stream::{Reader, Writer},
};
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Settings {
    #[default]
    System,
    Enabled,
    Disabled,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Type {
    #[default]
    None,
    Socks5,
    Http,
    Mtproto,
    Web,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Valid,
    Unsupported,
    IncorrectSecret,
    Invalid,
}
#[derive(Clone, Debug)]
pub struct ProxyData {
    /// QString null bits: host=1, user=2, password=4. Relevant only for empty text.
    pub string_nulls: u8,
    pub kind: Type,
    pub host: String,
    pub port: u32,
    pub user: String,
    pub password: String,
    pub resolved_ips: Vec<String>,
    pub resolved_expire_at: i64,
}
impl Default for ProxyData {
    fn default() -> Self {
        Self {
            kind: Type::None,
            host: String::new(),
            port: 0,
            user: String::new(),
            password: String::new(),
            resolved_ips: Vec::new(),
            resolved_expire_at: 0,
            string_nulls: 7,
        }
    }
}
impl PartialEq for ProxyData {
    fn eq(&self, other: &Self) -> bool {
        if !self.valid() {
            return !other.valid();
        }
        self.kind == other.kind
            && self.host == other.host
            && self.port == other.port
            && self.user == other.user
            && self.password == other.password
    }
}
impl Eq for ProxyData {}
fn is_hex(password: &str) -> bool {
    password.len() >= 32
        && password.len().is_multiple_of(2)
        && password.bytes().all(|c| c.is_ascii_hexdigit())
}
fn is_base64(password: &str) -> bool {
    let inner = password.strip_suffix('=').unwrap_or(password);
    let inner = inner.strip_suffix('=').unwrap_or(inner);
    password.len() >= 22
        && password.len() % 4 != 1
        && inner
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|p| Some((p[0] as char).to_digit(16)? as u8 * 16 + (p[1] as char).to_digit(16)? as u8))
        .collect()
}
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}
pub fn mtproto_password_status(password: &str) -> Status {
    if is_hex(password) {
        let size = password.len() / 2;
        if size == 16
            || size == 17 && password[..2].eq_ignore_ascii_case("dd")
            || size >= 21 && password[..2].eq_ignore_ascii_case("ee")
        {
            Status::Valid
        } else if size < 16 {
            Status::Invalid
        } else {
            Status::Unsupported
        }
    } else if is_base64(password) {
        let inner = password.trim_end_matches('=');
        let size = inner.len() * 3 / 4;
        let p = password.as_bytes();
        if size < 16 {
            Status::Invalid
        } else if size == 16
            || size == 17 && p[0] == b'3' && (b'Q'..=b'Z').contains(&p[1])
            || size == 17 && p[0] == b'3' && (b'a'..=b'f').contains(&p[1])
            || size >= 21 && p[0] == b'7' && (b'g'..=b'v').contains(&p[1])
        {
            Status::Valid
        } else if size >= 21 && password[..2].eq_ignore_ascii_case("ee") {
            Status::IncorrectSecret
        } else {
            Status::Unsupported
        }
    } else {
        Status::Invalid
    }
}
impl ProxyData {
    pub fn valid(&self) -> bool {
        self.status() == Status::Valid
    }
    pub fn status(&self) -> Status {
        if self.kind == Type::Web {
            if self.host.is_empty()
                || self.host != normalize_web_proxy_host(&self.host)
                || self.port != 443
                || self.user != normalize_web_proxy_base_path(&self.user)
            {
                return Status::Invalid;
            }
            let status = mtproto_password_status(&self.password);
            if status != Status::Valid {
                return status;
            }
            let secret = self.secret_from_mtproto_password();
            if secret.len() >= 21 && secret[0] == 0xee {
                Status::Unsupported
            } else {
                Status::Valid
            }
        } else if self.kind == Type::None || self.host.is_empty() || self.port == 0 {
            Status::Invalid
        } else if self.kind == Type::Mtproto {
            mtproto_password_status(&self.password)
        } else {
            Status::Valid
        }
    }
    pub fn supports_calls(&self) -> bool {
        false
    }
    pub fn try_custom_resolve(&self) -> bool {
        let dotted = self.host.split('.').collect::<Vec<_>>();
        let numeric = dotted.len() == 4
            && dotted
                .iter()
                .all(|v| !v.is_empty() && v.bytes().all(|c| c.is_ascii_digit()));
        matches!(self.kind, Type::Socks5 | Type::Mtproto)
            && self.host.parse::<std::net::Ipv6Addr>().is_err()
            && !numeric
    }
    pub fn secret_from_mtproto_password(&self) -> Vec<u8> {
        assert!(matches!(self.kind, Type::Mtproto | Type::Web));
        if is_hex(&self.password) {
            hex_decode(&self.password).unwrap_or_default()
        } else if is_base64(&self.password) {
            base64_decode(&self.password).unwrap_or_default()
        } else {
            Vec::new()
        }
    }
    pub fn web_base_path(&self) -> &str {
        if self.kind == Type::Web {
            &self.user
        } else {
            ""
        }
    }
    pub fn web_address(&self) -> String {
        if self.web_base_path().is_empty() {
            self.host.clone()
        } else {
            format!("{}/{}", self.host, self.user)
        }
    }
    pub fn set_web_address(&mut self, value: &str) {
        assert_eq!(self.kind, Type::Web);
        let input = value.trim();
        let input = if input
            .get(..8)
            .is_some_and(|s| s.eq_ignore_ascii_case("https://"))
        {
            &input[8..]
        } else {
            input
        };
        let (host, path) = input.split_once('/').unwrap_or((input, ""));
        self.host = normalize_web_proxy_host(host);
        let normalized = normalize_web_proxy_base_path(path);
        if self.host.is_empty() || !trim_base_path(path).is_empty() && normalized.is_empty() {
            self.host.clear();
            self.user.clear();
        } else {
            self.user = normalized;
        }
    }
    pub fn from_link(value: &str) -> Option<Self> {
        let url = reqwest::Url::parse(value).ok()?;
        if url.scheme() != "tg" {
            return None;
        }
        let kind = match url.host_str()? {
            "proxy" => Type::Mtproto,
            "socks" => Type::Socks5,
            _ => return None,
        };
        let mut query = std::collections::BTreeMap::new();
        for item in url.query()?.split('&') {
            let (k, v) = item.split_once('=').unwrap_or((item, ""));
            query
                .entry(percent_decode(k)?)
                .or_insert(percent_decode(v)?);
        }
        let result = Self {
            kind,
            host: query.get("server")?.clone(),
            port: query.get("port")?.parse().ok()?,
            user: query.get("user").cloned().unwrap_or_default(),
            password: query
                .get(if kind == Type::Mtproto {
                    "secret"
                } else {
                    "pass"
                })
                .cloned()
                .unwrap_or_default(),
            ..Self::default()
        };
        result.valid().then_some(result)
    }
}
pub fn normalize_web_proxy_host(value: &str) -> String {
    let input = value.trim();
    if input.is_empty() || input.contains([':', '/', '?', '#', '@']) || input.ends_with('.') {
        return String::new();
    }
    let Ok(url) = reqwest::Url::parse(&format!("https://{input}")) else {
        return String::new();
    };
    let Some(host) = url.domain() else {
        return String::new();
    };
    let host = host.to_ascii_lowercase();
    if host.len() > 253
        || !host.contains('.')
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
    {
        return String::new();
    }
    let last = host.rsplit('.').next().unwrap();
    let digits = last.strip_prefix("0x").unwrap_or(last);
    if digits.bytes().all(|c| {
        if last.starts_with("0x") {
            c.is_ascii_hexdigit()
        } else {
            c.is_ascii_digit()
        }
    }) {
        return String::new();
    }
    host
}
fn trim_base_path(value: &str) -> &str {
    let v = value.trim();
    let v = v.strip_prefix('/').unwrap_or(v);
    v.strip_suffix('/').unwrap_or(v)
}
pub fn normalize_web_proxy_base_path(value: &str) -> String {
    let path = trim_base_path(value);
    if path.is_empty() || path.len() > 128 {
        return String::new();
    }
    for segment in path.split('/') {
        if segment.is_empty()
            || !segment.as_bytes()[0].is_ascii_alphanumeric()
            || !segment
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return String::new();
        }
    }
    path.into()
}
pub fn web_proxy_bridge_capability(proxy: &ProxyData) -> String {
    assert_eq!(proxy.kind, Type::Web);
    assert_eq!(proxy.host, normalize_web_proxy_host(&proxy.host));
    assert_eq!(
        proxy.web_base_path(),
        normalize_web_proxy_base_path(proxy.web_base_path())
    );
    let secret = proxy.secret_from_mtproto_password();
    assert!(!secret.is_empty());
    let context = if proxy.web_base_path().is_empty() {
        format!("tdesktop-web-proxy-bridge-v1\n{}", proxy.host)
    } else {
        format!(
            "tdesktop-web-proxy-bridge-v2\n{}\n{}",
            proxy.host,
            proxy.web_base_path()
        )
    };
    base64_url_encode(&tg_base::openssl_help::hmac_sha256(
        &secret,
        context.as_bytes(),
    ))
}
pub fn encode_web_proxy_link_secret(proxy: &ProxyData) -> String {
    let secret = proxy.secret_from_mtproto_password();
    if proxy.web_base_path().is_empty() || secret.is_empty() {
        return proxy.password.clone();
    }
    let mut marked = vec![0x70];
    marked.extend(secret);
    base64_url_encode(&marked)
}
pub fn decode_web_proxy_link_secret(value: &str, has_base_path: bool) -> String {
    if let Some(decoded) = base64_decode(value) {
        if decoded.len() >= 17 && decoded[0] == 0x70 {
            return hex_encode(&decoded[1..]);
        }
    }
    if has_base_path {
        String::new()
    } else {
        value.into()
    }
}
pub fn web_proxy_bridge_path(proxy: &ProxyData) -> String {
    if proxy.web_base_path().is_empty() {
        "/".into()
    } else {
        format!("/{}/", proxy.web_base_path())
    }
}
pub fn web_proxy_bridge_url(proxy: &ProxyData) -> String {
    format!(
        "https://{}{}?bridge={}",
        proxy.host,
        web_proxy_bridge_path(proxy),
        web_proxy_bridge_capability(proxy)
    )
}
pub fn to_direct_ip_proxy(proxy: &ProxyData, index: usize) -> ProxyData {
    if !proxy.try_custom_resolve() {
        return proxy.clone();
    }
    let Some(ip) = proxy.resolved_ips.get(index) else {
        return proxy.clone();
    };
    ProxyData {
        host: ip.clone(),
        resolved_ips: Vec::new(),
        resolved_expire_at: 0,
        ..proxy.clone()
    }
}
fn percent_decode(value: &str) -> Option<String> {
    let mut out = Vec::new();
    let b = value.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            out.push(
                (b.get(i + 1).copied()? as char).to_digit(16)? as u8 * 16
                    + (b.get(i + 2).copied()? as char).to_digit(16)? as u8,
            );
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}
/// Per-proxy record used by current core/proxy_settings: type, QString host, port,
/// QString user, QString password. The enclosing settings block belongs to stage 3.
pub fn serialize_data(p: &ProxyData) -> Vec<u8> {
    let mut out = Writer::default();
    out.i32(p.kind as i32);
    write_string(&mut out, &p.host, p.string_nulls & 1 != 0);
    out.i32(p.port as i32);
    write_string(&mut out, &p.user, p.string_nulls & 2 != 0);
    write_string(&mut out, &p.password, p.string_nulls & 4 != 0);
    out.0
}
fn write_string(out: &mut Writer, value: &str, null: bool) {
    if null && value.is_empty() {
        out.i32(-1);
    } else {
        out.string(value);
    }
}
fn read_string(input: &mut Reader<'_>, nulls: &mut u8, bit: u8) -> Option<String> {
    let is_null = input.data.get(input.position..input.position + 4)? == [255; 4];
    if is_null {
        *nulls |= bit;
    }
    input.string()
}
pub fn deserialize_data(bytes: &[u8]) -> Option<ProxyData> {
    let mut input = Reader::new(bytes);
    let kind = match input.i32()? {
        1 => Type::Socks5,
        2 => Type::Http,
        3 => Type::Mtproto,
        4 => Type::Web,
        _ => Type::None,
    };
    let mut nulls = 0;
    let host = read_string(&mut input, &mut nulls, 1)?;
    let port = input.i32()? as u32;
    let user = read_string(&mut input, &mut nulls, 2)?;
    let password = read_string(&mut input, &mut nulls, 4)?;
    Some(ProxyData {
        kind,
        host,
        port,
        user,
        password,
        string_nulls: nulls,
        ..ProxyData::default()
    })
}
pub fn serialize_list(list: &[ProxyData]) -> Vec<u8> {
    let mut out = Writer::default();
    out.i32(list.len() as i32);
    for proxy in list {
        out.bytes(&serialize_data(proxy));
    }
    out.0
}
pub fn deserialize_list(bytes: &[u8]) -> Option<Vec<ProxyData>> {
    let mut input = Reader::new(bytes);
    let count = input.i32()?;
    if count < 0 || count as usize > bytes.len() / 4 {
        return None;
    }
    let mut list = Vec::new();
    for _ in 0..count {
        list.push(deserialize_data(&input.bytes()?)?);
    }
    input.at_end().then_some(list)
}
