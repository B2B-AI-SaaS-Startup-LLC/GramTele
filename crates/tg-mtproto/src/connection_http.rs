//! MTProto HTTP over the same byte streams/proxy handshakes as TCP.
//! A request owns a connection until its response is read; no HTTP pipelining.
use crate::{
    connection_abstract::{Clock, ConnectionState, Event, Exchange, TransportError},
    core_types::{Buffer, Random},
    details::{mtproto_abstract_socket::ByteStream, mtproto_tcp_socket::connect_to_host},
    mtproto_proxy_data::ProxyData,
};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
pub const CONTENT_TYPE: &str = "application/x-www-form-urlencoded";
pub const MAX_HTTP_BODY: usize = 64 * 1024 * 1024;
pub type HttpStream = Box<dyn ByteStream + Send>;
pub type ConnectFuture<'a> =
    Pin<Box<dyn Future<Output = Result<HttpStream, TransportError>> + Send + 'a>>;
pub trait HttpConnector: Send + Sync {
    fn connect(&self) -> ConnectFuture<'_>;
}
pub struct NetworkConnector {
    pub address: String,
    pub proxy: ProxyData,
}
impl HttpConnector for NetworkConnector {
    fn connect(&self) -> ConnectFuture<'_> {
        Box::pin(async move {
            let stream = connect_to_host(&self.address, 80, &self.proxy, false).await?;
            Ok(Box::new(stream) as HttpStream)
        })
    }
}
pub fn url(address: &str) -> String {
    format!("http://{}:80/api", host_header(address))
}
fn host_header(address: &str) -> String {
    if address.parse::<std::net::Ipv6Addr>().is_ok() {
        format!("[{address}]")
    } else {
        address.into()
    }
}
pub fn request_body(buffer: &[i32]) -> Result<Vec<u8>, TransportError> {
    if buffer.len() <= 2 {
        return Err(TransportError::Protocol("missing HTTP packet body"));
    }
    Ok(tg_tl::primes_to_bytes(&buffer[2..]))
}
pub fn handle_response(bytes: &[u8]) -> Result<Buffer, TransportError> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if !bytes.len().is_multiple_of(4) || bytes.len() < 8 {
        return Err(TransportError::Server(-500));
    }
    Ok(tg_tl::bytes_to_primes(bytes))
}
/// Default headers captured from QNetworkAccessManager Qt 6.11.2 on Linux.
/// Compression capabilities and locale are supplied by the embedding process.
#[derive(Clone, Debug)]
pub struct HttpHeaders {
    pub accept_encoding: String,
    pub accept_language: String,
    pub user_agent: String,
}
impl Default for HttpHeaders {
    fn default() -> Self {
        Self {
            accept_encoding: "zstd, br, gzip, deflate".into(),
            accept_language: "en,*".into(),
            user_agent: "Mozilla/5.0".into(),
        }
    }
}
pub fn encode_request(
    host: &str,
    body: &[u8],
    headers: &HttpHeaders,
) -> Result<Vec<u8>, TransportError> {
    if [
        host,
        &headers.accept_encoding,
        &headers.accept_language,
        &headers.user_agent,
    ]
    .iter()
    .any(|v| v.contains(['\r', '\n']))
    {
        return Err(TransportError::Protocol("newline in HTTP request header"));
    }
    let mut request=format!("POST /api HTTP/1.1\r\nHost: {}\r\nContent-Length: {}\r\nContent-Type: {CONTENT_TYPE}\r\nConnection: Keep-Alive\r\nAccept-Encoding: {}\r\nAccept-Language: {}\r\nUser-Agent: {}\r\n\r\n",host_header(host),body.len(),headers.accept_encoding,headers.accept_language,headers.user_agent).into_bytes();
    request.extend_from_slice(body);
    Ok(request)
}
pub struct HttpReply {
    pub status: u16,
    pub body: Vec<u8>,
    pub keep_alive: bool,
}
async fn line<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, TransportError> {
    let mut out = Vec::new();
    loop {
        let bytes = reader.fill_buf().await?;
        if bytes.is_empty() {
            return Err(TransportError::Protocol("EOF in HTTP headers"));
        }
        let n = bytes
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes.len(), |i| i + 1);
        if out.len() + n > 65536 {
            return Err(TransportError::Protocol("HTTP line exceeds 64 KiB"));
        }
        out.extend_from_slice(&bytes[..n]);
        reader.consume(n);
        if out.last() == Some(&b'\n') {
            out.pop();
            if out.last() == Some(&b'\r') {
                out.pop();
            }
            return Ok(out);
        }
    }
}
pub async fn read_response<R: AsyncBufRead + Unpin>(
    reader: &mut R,
) -> Result<HttpReply, TransportError> {
    loop {
        let status = line(reader).await?;
        let text = std::str::from_utf8(&status)
            .map_err(|_| TransportError::Protocol("non-ASCII HTTP status"))?;
        let parts: Vec<_> = text.split_ascii_whitespace().collect();
        if parts.len() < 2 || !matches!(parts[0], "HTTP/1.0" | "HTTP/1.1") || parts[1].len() != 3 {
            return Err(TransportError::Protocol("bad HTTP status line"));
        }
        let code = parts[1]
            .parse::<u16>()
            .map_err(|_| TransportError::Protocol("bad HTTP status code"))?;
        if !(100..=599).contains(&code) {
            return Err(TransportError::Protocol("bad HTTP status code"));
        }
        let mut size = status.len();
        let mut headers = Vec::new();
        loop {
            let bytes = line(reader).await?;
            size += bytes.len() + 2;
            if size > 65536 {
                return Err(TransportError::Protocol("HTTP headers exceed 64 KiB"));
            }
            if bytes.is_empty() {
                break;
            }
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| TransportError::Protocol("bad HTTP header encoding"))?;
            let (name, value) = text
                .split_once(':')
                .ok_or(TransportError::Protocol("bad HTTP header"))?;
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
        if (100..200).contains(&code) && code != 101 {
            continue;
        }
        let value = |name: &str| {
            headers
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.as_str())
        };
        let connection = value("connection").unwrap_or("");
        let mut keep_alive = if parts[0] == "HTTP/1.1" {
            !connection.eq_ignore_ascii_case("close")
        } else {
            connection.eq_ignore_ascii_case("keep-alive")
        };
        let mut body = Vec::new();
        if code != 204 && code != 304 {
            if value("transfer-encoding").is_some_and(|s| s.eq_ignore_ascii_case("chunked")) {
                loop {
                    let chunk = line(reader).await?;
                    let text = std::str::from_utf8(&chunk)
                        .map_err(|_| TransportError::Protocol("bad chunk size"))?;
                    let n = usize::from_str_radix(
                        text.split(';').next().unwrap_or_default().trim(),
                        16,
                    )
                    .map_err(|_| TransportError::Protocol("bad chunk size"))?;
                    if n == 0 {
                        loop {
                            let trailer = line(reader).await?;
                            size += trailer.len() + 2;
                            if size > 65536 {
                                return Err(TransportError::Protocol(
                                    "HTTP trailers exceed 64 KiB",
                                ));
                            }
                            if trailer.is_empty() {
                                break;
                            }
                        }
                        break;
                    }
                    if n > MAX_HTTP_BODY - body.len() {
                        return Err(TransportError::Protocol("HTTP body exceeds limit"));
                    }
                    let old = body.len();
                    body.resize(old + n, 0);
                    reader.read_exact(&mut body[old..]).await?;
                    if !line(reader).await?.is_empty() {
                        return Err(TransportError::Protocol("bad HTTP chunk terminator"));
                    }
                }
            } else if let Some(length) = value("content-length") {
                let n = length
                    .parse::<usize>()
                    .map_err(|_| TransportError::Protocol("bad HTTP content length"))?;
                if n > MAX_HTTP_BODY {
                    return Err(TransportError::Protocol("HTTP body exceeds limit"));
                }
                if headers
                    .iter()
                    .filter(|(k, _)| k == "content-length")
                    .any(|(_, v)| v != length)
                {
                    return Err(TransportError::Protocol("conflicting HTTP content lengths"));
                }
                body.resize(n, 0);
                reader.read_exact(&mut body).await?;
            } else {
                keep_alive = false;
                reader
                    .take((MAX_HTTP_BODY + 1) as u64)
                    .read_to_end(&mut body)
                    .await?;
                if body.len() > MAX_HTTP_BODY {
                    return Err(TransportError::Protocol("HTTP body exceeds limit"));
                }
            }
        }
        match value("content-encoding")
            .unwrap_or("identity")
            .to_ascii_lowercase()
            .as_str()
        {
            "identity" | "" => {}
            "gzip" => body = tg_base::zlib_help::inflate_gzip(&body, MAX_HTTP_BODY)?,
            _ => {
                return Err(TransportError::Protocol(
                    "unsupported HTTP content encoding",
                ))
            }
        }
        return Ok(HttpReply {
            status: code,
            body,
            keep_alive,
        });
    }
}
type PooledStream = BufReader<HttpStream>;
struct Inner {
    host: String,
    connector: Arc<dyn HttpConnector>,
    idle: Mutex<Vec<PooledStream>>,
    permits: Arc<tokio::sync::Semaphore>,
    pending: Arc<AtomicUsize>,
    cancel: tokio::sync::watch::Sender<bool>,
    headers: HttpHeaders,
}
/// Clones are handles of one manager. Like Qt's default HTTP/1 manager, six
/// requests may be in flight; an idle keep-alive connection is reused.
#[derive(Clone)]
pub struct HttpConnection {
    inner: Arc<Inner>,
    pub state: Arc<Mutex<ConnectionState>>,
}
struct Pending(Arc<AtomicUsize>);
impl Drop for Pending {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}
impl HttpConnection {
    pub fn new(address: &str, proxy: ProxyData) -> Self {
        Self::from_connector(
            address.into(),
            Arc::new(NetworkConnector {
                address: address.into(),
                proxy,
            }),
            HttpHeaders::default(),
        )
    }
    pub fn from_connector(
        host: String,
        connector: Arc<dyn HttpConnector>,
        headers: HttpHeaders,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                host,
                connector,
                idle: Mutex::new(Vec::new()),
                permits: Arc::new(tokio::sync::Semaphore::new(6)),
                pending: Arc::new(AtomicUsize::new(0)),
                cancel: tokio::sync::watch::channel(false).0,
                headers,
            }),
            state: Arc::new(Mutex::new(ConnectionState::default())),
        }
    }
    pub fn using_http_wait(&self) -> bool {
        true
    }
    pub fn need_http_wait(&self) -> bool {
        !*self.inner.cancel.borrow() && self.inner.pending.load(Ordering::Relaxed) == 0
    }
    pub fn disconnect_from_server(&self) {
        self.inner.cancel.send_replace(true);
        self.inner.idle.lock().unwrap().clear();
        self.state.lock().unwrap().disconnect();
    }
    pub async fn send_data(&self, packet: Buffer) -> Result<Buffer, TransportError> {
        let mut cancel = self.inner.cancel.subscribe();
        if *cancel.borrow() {
            return Err(TransportError::Protocol("HTTP connection finished"));
        }
        let body = request_body(&packet)?;
        let request = encode_request(&self.inner.host, &body, &self.inner.headers)?;
        self.inner.pending.fetch_add(1, Ordering::Relaxed);
        let _pending = Pending(self.inner.pending.clone());
        let operation = async {
            let _permit = self
                .inner
                .permits
                .acquire()
                .await
                .map_err(|_| TransportError::Protocol("HTTP manager closed"))?;
            let idle = self.inner.idle.lock().unwrap().pop();
            let mut stream = if let Some(stream) = idle {
                stream
            } else {
                BufReader::new(self.inner.connector.connect().await?)
            };
            stream.write_all(&request).await?;
            stream.flush().await?;
            let response = read_response(&mut stream).await?;
            if response.keep_alive && !*self.inner.cancel.borrow() {
                self.inner.idle.lock().unwrap().push(stream);
            }
            if !(200..300).contains(&response.status) {
                return Err(TransportError::Server(-(response.status as i32)));
            }
            handle_response(&response.body)
        };
        tokio::select! {result=operation=>result,_=cancel.changed()=>Err(TransportError::Protocol("HTTP request cancelled"))}
    }
    pub async fn probe(
        &self,
        id: u64,
        clock: &impl Clock,
        random: &mut impl Random,
    ) -> Result<(), TransportError> {
        let nonce = tg_tl::Int128 {
            l: random.u64(),
            h: random.u64(),
        };
        let start = clock.now().monotonic_ms;
        let packet = crate::connection_abstract::prepare_not_secure_packet(
            &tg_scheme::ReqPq::new(nonce),
            id,
            random,
        );
        let response = self.send_data(packet).await?;
        let body = crate::connection_abstract::parse_not_secure_response(&response)?;
        let mut reader = tg_tl::Reader::new(body);
        let parsed = <tg_scheme::ResPQ as tg_tl::Deserialize>::read(&mut reader)
            .map_err(|_| TransportError::Protocol("Could not parse HTTP fake pq-response"))?;
        if parsed.c_res_pq().nonce != nonce {
            return Err(TransportError::Protocol(
                "Wrong nonce in HTTP fake pq-response",
            ));
        }
        let mut state = self.state.lock().unwrap();
        state.connected = true;
        state.ping_time = clock.now().monotonic_ms - start;
        state.events.push_back(Event::Connected);
        Ok(())
    }
    pub async fn receive_into_queue(&self, packet: Buffer) -> Result<(), TransportError> {
        let response = self.send_data(packet).await?;
        if !response.is_empty() {
            self.state.lock().unwrap().push_received(response);
        }
        Ok(())
    }
}
impl Exchange for HttpConnection {
    async fn exchange(&mut self, packet: Buffer) -> Result<Buffer, TransportError> {
        self.send_data(packet).await
    }
}
