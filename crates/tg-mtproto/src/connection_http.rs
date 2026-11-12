//! Original POST /api, concurrent pending requests, cancellation and http_wait.
use crate::{
    connection_abstract::{Clock, ConnectionState, Event, Exchange, TransportError},
    core_types::{Buffer, Random},
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
pub const CONTENT_TYPE: &str = "application/x-www-form-urlencoded";
pub fn url(address: &str) -> String {
    if address.parse::<std::net::Ipv6Addr>().is_ok() {
        format!("http://[{address}]:80/api")
    } else {
        format!("http://{address}:80/api")
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
/// Clones are request handles for the same manager; cancellation affects them all.
#[derive(Clone)]
pub struct HttpConnection {
    client: reqwest::Client,
    url: String,
    pending: Arc<AtomicUsize>,
    cancel: tokio::sync::watch::Sender<bool>,
    pub state: Arc<Mutex<ConnectionState>>,
}
struct Pending(Arc<AtomicUsize>);
impl Drop for Pending {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}
impl HttpConnection {
    pub fn new(client: reqwest::Client, address: &str) -> Self {
        Self {
            client,
            url: url(address),
            pending: Arc::new(AtomicUsize::new(0)),
            cancel: tokio::sync::watch::channel(false).0,
            state: Arc::new(Mutex::new(ConnectionState::default())),
        }
    }
    pub fn using_http_wait(&self) -> bool {
        true
    }
    pub fn need_http_wait(&self) -> bool {
        !*self.cancel.borrow() && self.pending.load(Ordering::Relaxed) == 0
    }
    pub fn disconnect_from_server(&self) {
        self.cancel.send_replace(true);
        self.state.lock().unwrap().disconnect();
    }
    pub async fn send_data(&self, packet: Buffer) -> Result<Buffer, TransportError> {
        let mut cancel = self.cancel.subscribe();
        if *cancel.borrow() {
            return Err(TransportError::Protocol("HTTP connection finished"));
        }
        let body = request_body(&packet)?;
        self.pending.fetch_add(1, Ordering::Relaxed);
        let _pending = Pending(self.pending.clone());
        let operation = async {
            let response = self
                .client
                .post(&self.url)
                .header(reqwest::header::CONTENT_TYPE, CONTENT_TYPE)
                .body(body)
                .send()
                .await?;
            if !response.status().is_success() {
                return Err(TransportError::Server(-(response.status().as_u16() as i32)));
            }
            handle_response(&response.bytes().await?)
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
