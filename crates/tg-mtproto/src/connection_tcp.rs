//! Original versions 0/1/D framing and obfuscated stream, independent of sockets.
use crate::{
    connection_abstract::{Exchange, TransportError},
    core_types::{Buffer, Random},
    mtproto_auth_key::CtrState,
};
use tg_base::openssl_help::sha256_parts;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
pub const PACKET_SIZE_MAX: usize = 0x1000000 * 4;
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Protocol {
    Version0,
    Version1([u8; 16]),
    VersionD([u8; 16]),
}
impl Protocol {
    pub fn from_secret(secret: &[u8]) -> Result<Self, TransportError> {
        match secret.len() {
            0 => Ok(Self::Version0),
            16 => Ok(Self::Version1(secret.try_into().unwrap())),
            17 if secret[0] == 0xdd => Ok(Self::VersionD(secret[1..17].try_into().unwrap())),
            n if n >= 21 && secret[0] == 0xee => {
                Ok(Self::VersionD(secret[1..17].try_into().unwrap()))
            }
            _ => Err(TransportError::Protocol("unsupported transport secret")),
        }
    }
    pub fn id(&self) -> u32 {
        if matches!(self, Self::VersionD(_)) {
            0xdddddddd
        } else {
            0xefefefef
        }
    }
    pub fn supports_arbitrary_length(&self) -> bool {
        matches!(self, Self::VersionD(_))
    }
    fn prepare_key(&self, source: &[u8]) -> [u8; 32] {
        match self {
            Self::Version0 => source.try_into().unwrap(),
            Self::Version1(s) | Self::VersionD(s) => sha256_parts(&[source, s]).try_into().unwrap(),
        }
    }
    pub fn frame(&self, body: &[u8], random: &mut impl Random) -> Result<Vec<u8>, TransportError> {
        if body.is_empty() || !body.len().is_multiple_of(4) || body.len() >= PACKET_SIZE_MAX {
            return Err(TransportError::Protocol("bad outgoing packet length"));
        }
        let mut out = Vec::new();
        if self.supports_arbitrary_length() {
            let padding = (random.u32() & 15) as usize;
            out.extend(((body.len() + padding) as u32).to_le_bytes());
            out.extend_from_slice(body);
            let mut bytes = vec![0; padding.div_ceil(4) * 4];
            for word in bytes.chunks_exact_mut(4) {
                word.copy_from_slice(&random.u32().to_le_bytes());
            }
            out.extend_from_slice(&bytes[..padding]);
        } else {
            let count = body.len() / 4;
            if count < 127 {
                out.push(count as u8);
            } else {
                out.push(127);
                out.extend_from_slice(&(count as u32).to_le_bytes()[..3]);
            }
            out.extend_from_slice(body);
        }
        Ok(out)
    }
    /// None means incomplete header, not an invalid frame.
    pub fn packet_length(&self, bytes: &[u8]) -> Result<Option<(usize, usize)>, TransportError> {
        let invalid = || TransportError::Protocol("invalid TCP packet length");
        if self.supports_arbitrary_length() {
            if bytes.len() < 4 {
                return Ok(None);
            }
            let size = u32::from_le_bytes(bytes[..4].try_into().unwrap()).wrapping_add(4) as usize;
            if !(8..PACKET_SIZE_MAX).contains(&size) {
                return Err(invalid());
            }
            Ok(Some((size, 4)))
        } else {
            let Some(&first) = bytes.first() else {
                return Ok(None);
            };
            if first == 127 {
                if bytes.len() < 4 {
                    return Ok(None);
                }
                let count = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], 0]) as usize;
                if count < 127 {
                    return Err(invalid());
                }
                Ok(Some((count * 4 + 4, 4)))
            } else if (1..127).contains(&first) {
                Ok(Some((first as usize * 4 + 1, 1)))
            } else {
                Err(invalid())
            }
        }
    }
}
pub fn is_good_start_nonce(nonce: &[u8; 64]) -> bool {
    let first = u32::from_le_bytes(nonce[..4].try_into().unwrap());
    nonce[0] != 0xef
        && !matches!(
            first,
            0x44414548 | 0x54534f50 | 0x20544547 | 0xeeeeeeee | 0xdddddddd | 0x02010316
        )
        && nonce[4..8] != [0; 4]
}
/// Outgoing CTR is advanced over all 64 bytes even though only the last 8 are sent encrypted.
pub fn obfuscated_header(
    protocol: &Protocol,
    mut nonce: [u8; 64],
    dc_id: i16,
) -> ([u8; 64], CtrState, CtrState) {
    let send_key = protocol.prepare_key(&nonce[8..40]);
    let send_iv = nonce[40..56].try_into().unwrap();
    let mut reverse = nonce[8..56].to_vec();
    reverse.reverse();
    let receive_key = protocol.prepare_key(&reverse[..32]);
    let receive_iv = reverse[32..].try_into().unwrap();
    let mut send = CtrState::new(&send_key, &send_iv);
    let receive = CtrState::new(&receive_key, &receive_iv);
    nonce[56..60].copy_from_slice(&protocol.id().to_le_bytes());
    nonce[60..62].copy_from_slice(&dc_id.to_le_bytes());
    let mut header = nonce;
    send.apply(&mut nonce);
    header[56..].copy_from_slice(&nonce[56..]);
    (header, send, receive)
}
pub struct TcpCodec {
    pub protocol: Protocol,
    send: CtrState,
    receive: CtrState,
    buffer: Vec<u8>,
}
impl TcpCodec {
    pub fn start(protocol: Protocol, dc_id: i16, random: &mut impl Random) -> ([u8; 64], Self) {
        Self::start_with_filter(protocol, dc_id, random, is_good_start_nonce)
    }
    pub fn start_with_filter(
        protocol: Protocol,
        dc_id: i16,
        random: &mut impl Random,
        mut good: impl FnMut(&[u8; 64]) -> bool,
    ) -> ([u8; 64], Self) {
        let mut nonce = [0; 64];
        loop {
            random.fill(&mut nonce);
            if good(&nonce) {
                break;
            }
        }
        let (header, send, receive) = obfuscated_header(&protocol, nonce, dc_id);
        (
            header,
            Self {
                protocol,
                send,
                receive,
                buffer: Vec::new(),
            },
        )
    }
    pub fn encode(
        &mut self,
        body: &[u8],
        random: &mut impl Random,
    ) -> Result<Vec<u8>, TransportError> {
        let mut bytes = self.protocol.frame(body, random)?;
        self.send.apply(&mut bytes);
        Ok(bytes)
    }
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Buffer>, TransportError> {
        decode_frames(&self.protocol, &mut self.receive, &mut self.buffer, bytes)
    }
}
fn decode_frames(
    protocol: &Protocol,
    receive: &mut CtrState,
    buffer: &mut Vec<u8>,
    bytes: &[u8],
) -> Result<Vec<Buffer>, TransportError> {
    let mut bytes = bytes.to_vec();
    receive.apply(&mut bytes);
    buffer.extend(bytes);
    let mut packets = Vec::new();
    while let Some((size, prefix)) = protocol.packet_length(buffer)? {
        if buffer.len() < size {
            break;
        }
        let body = &buffer[prefix..size];
        let words = body.len() / 4;
        let mut packet = tg_tl::bytes_to_primes(&body[..words * 4]);
        if words < 3 {
            packet.truncate(1);
        }
        packets.push(packet);
        buffer.drain(..size);
    }
    Ok(packets)
}
/// Any Tokio byte stream works, including duplex streams used by memory tests.
pub struct TcpConnection<S, R> {
    stream: S,
    pub state: crate::connection_abstract::ConnectionState,
    header: Option<[u8; 64]>,
    codec: TcpCodec,
    random: R,
}
impl<S: AsyncRead + AsyncWrite + Unpin, R: Random> TcpConnection<S, R> {
    pub async fn new(
        stream: S,
        protocol: Protocol,
        dc_id: i16,
        random: R,
    ) -> Result<Self, TransportError> {
        Self::new_with_nonce_filter(stream, protocol, dc_id, random, is_good_start_nonce).await
    }
    /// Fake TLS accepts every obfuscation nonce; TCP and WEB use the filter.
    pub async fn new_with_nonce_filter(
        stream: S,
        protocol: Protocol,
        dc_id: i16,
        mut random: R,
        filter: impl FnMut(&[u8; 64]) -> bool,
    ) -> Result<Self, TransportError> {
        let (header, codec) = TcpCodec::start_with_filter(protocol, dc_id, &mut random, filter);
        Ok(Self {
            stream,
            state: Default::default(),
            header: Some(header),
            codec,
            random,
        })
    }
    pub async fn send_data(&mut self, packet: Buffer) -> Result<(), TransportError> {
        if packet.len() <= 2 {
            return Err(TransportError::Protocol("missing packet body"));
        }
        let body = tg_tl::primes_to_bytes(&packet[2..]);
        let frame = self.codec.encode(&body, &mut self.random)?;
        let bytes = if let Some(header) = self.header.take() {
            [header.as_slice(), frame.as_slice()].concat()
        } else {
            frame
        };
        self.stream.write_all(&bytes).await?;
        Ok(())
    }
    pub async fn receive(&mut self) -> Result<Buffer, TransportError> {
        if self.state.received.is_empty() {
            self.receive_into_queue().await?;
        }
        Ok(self.state.received.pop_front().expect("received packet"))
    }
    /// Original fake req_pq probe; call before handing the connection to key creation.
    pub async fn probe(
        &mut self,
        id: u64,
        clock: &impl crate::connection_abstract::Clock,
    ) -> Result<(), TransportError> {
        let nonce = tg_tl::Int128 {
            l: self.random.u64(),
            h: self.random.u64(),
        };
        let start = clock.now().monotonic_ms;
        let packet = crate::connection_abstract::prepare_not_secure_packet(
            &tg_scheme::ReqPq::new(nonce),
            id,
            &mut self.random,
        );
        self.send_data(packet).await?;
        let response = self.receive().await?;
        let body = crate::connection_abstract::parse_not_secure_response(&response)?;
        let mut reader = tg_tl::Reader::new(body);
        let parsed = <tg_scheme::ResPQ as tg_tl::Deserialize>::read(&mut reader)
            .map_err(|_| TransportError::Protocol("Could not parse TCP fake pq-response"))?;
        if parsed.c_res_pq().nonce != nonce {
            return Err(TransportError::Protocol(
                "Wrong nonce in TCP fake pq-response",
            ));
        }
        self.state.connected = true;
        self.state.ping_time = clock.now().monotonic_ms - start;
        self.state
            .events
            .push_back(crate::connection_abstract::Event::Connected);
        Ok(())
    }
    pub async fn receive_into_queue(&mut self) -> Result<(), TransportError> {
        receive_packets(
            &mut self.stream,
            &self.codec.protocol,
            &mut self.codec.receive,
            &mut self.codec.buffer,
            &mut self.state,
        )
        .await
    }
    /// Separate read and write ownership. CTR counters, buffered partial frames,
    /// queued responses, and the unsent obfuscation header are moved unchanged.
    pub fn into_split(
        self,
    ) -> (
        TcpReader<tokio::io::ReadHalf<S>>,
        TcpWriter<tokio::io::WriteHalf<S>, R>,
    ) {
        let (read, write) = tokio::io::split(self.stream);
        (
            TcpReader {
                stream: read,
                protocol: self.codec.protocol.clone(),
                receive: self.codec.receive,
                buffer: self.codec.buffer,
                state: self.state,
            },
            TcpWriter {
                stream: write,
                protocol: self.codec.protocol,
                send: self.codec.send,
                header: self.header,
                random: self.random,
            },
        )
    }
    pub async fn disconnect_from_server(&mut self) -> Result<(), TransportError> {
        self.stream.shutdown().await?;
        self.state.disconnect();
        Ok(())
    }
}
impl<S: AsyncRead + AsyncWrite + Unpin, R: Random> Exchange for TcpConnection<S, R> {
    async fn exchange(&mut self, packet: Buffer) -> Result<Buffer, TransportError> {
        self.send_data(packet).await?;
        self.receive().await
    }
}

/// Independent receiving half. Cancelling `receive()` while waiting for bytes is
/// safe: partial frames already read stay in this owner's buffer.
pub struct TcpReader<S> {
    stream: S,
    protocol: Protocol,
    receive: CtrState,
    buffer: Vec<u8>,
    pub state: crate::connection_abstract::ConnectionState,
}
impl<S: AsyncRead + Unpin> TcpReader<S> {
    pub async fn receive(&mut self) -> Result<Buffer, TransportError> {
        if self.state.received.is_empty() {
            receive_packets(
                &mut self.stream,
                &self.protocol,
                &mut self.receive,
                &mut self.buffer,
                &mut self.state,
            )
            .await?;
        }
        Ok(self.state.received.pop_front().expect("received packet"))
    }
}
pub struct TcpWriter<S, R> {
    stream: S,
    protocol: Protocol,
    send: CtrState,
    header: Option<[u8; 64]>,
    random: R,
}
impl<S: AsyncWrite + Unpin, R: Random> TcpWriter<S, R> {
    /// Must run to completion, or discard the connection: AsyncWrite may already
    /// have sent part of a frame when its future is cancelled.
    pub async fn send_data(&mut self, packet: Buffer) -> Result<(), TransportError> {
        if packet.len() <= 2 {
            return Err(TransportError::Protocol("missing packet body"));
        }
        let mut frame = self
            .protocol
            .frame(&tg_tl::primes_to_bytes(&packet[2..]), &mut self.random)?;
        self.send.apply(&mut frame);
        let bytes = if let Some(header) = self.header.take() {
            [header.as_slice(), frame.as_slice()].concat()
        } else {
            frame
        };
        self.stream.write_all(&bytes).await?;
        Ok(())
    }
    pub async fn disconnect_from_server(&mut self) -> Result<(), TransportError> {
        self.stream.shutdown().await?;
        Ok(())
    }
}
async fn receive_packets<S: AsyncRead + Unpin>(
    stream: &mut S,
    protocol: &Protocol,
    receive: &mut CtrState,
    buffer: &mut Vec<u8>,
    state: &mut crate::connection_abstract::ConnectionState,
) -> Result<(), TransportError> {
    use crate::connection_abstract::Event;
    loop {
        let mut bytes = [0; 8192];
        let count = match stream.read(&mut bytes).await {
            Ok(count) => count,
            Err(error) => {
                state.events.push_back(Event::Error(-499));
                return Err(error.into());
            }
        };
        if count == 0 {
            state.disconnect();
            return Err(TransportError::Protocol("TCP peer closed connection"));
        }
        let packets = match decode_frames(protocol, receive, buffer, &bytes[..count]) {
            Ok(packets) => packets,
            Err(error) => {
                state.events.push_back(Event::Error(-499));
                return Err(error);
            }
        };
        for packet in packets {
            if packet.len() == 1 {
                if packet[0] != 0 {
                    state.events.push_back(Event::Error(packet[0]));
                    return Err(TransportError::Server(packet[0]));
                }
            } else {
                state.push_received(packet);
            }
        }
        if !buffer.is_empty() {
            state.events.push_back(Event::ReceivedSome);
        }
        if !state.received.is_empty() {
            return Ok(());
        }
    }
}
