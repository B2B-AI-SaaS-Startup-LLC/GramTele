//! Fake TLS rule interpreter and record framing. No real TLS/ECDH session exists.
use crate::{connection_abstract::TransportError, core_types::Random};
use tg_scheme::TlsBlock;
/// X25519 public-key generation and permutation are explicit entropy inputs.
/// A production provider requires a separately approved X25519 implementation.
pub trait HelloEntropy: Random {
    fn public_key(&mut self) -> Result<[u8; 32], TransportError>;
    fn permutation(&mut self, length: usize) -> Vec<usize>;
}
pub struct ClientHello {
    pub data: Vec<u8>,
    pub digest: [u8; 32],
}
struct Part<'a, E> {
    domain: &'a [u8],
    greases: &'a [u8; 8],
    entropy: &'a mut E,
    result: Vec<u8>,
    digest_position: Option<usize>,
}
impl<E: HelloEntropy> Part<'_, E> {
    fn append(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if bytes.is_empty() || self.result.len() + bytes.len() > 2048 {
            return Err(TransportError::Protocol("Could not generate Client Hello."));
        }
        self.result.extend_from_slice(bytes);
        Ok(())
    }
    fn zeros(&mut self, length: usize) -> Result<(), TransportError> {
        if length == 32 && self.digest_position.is_none() {
            self.digest_position = Some(self.result.len());
        }
        self.append(&vec![0; length])
    }
    fn blocks(&mut self, blocks: &[TlsBlock]) -> Result<(), TransportError> {
        for block in blocks {
            self.block(block)?;
        }
        Ok(())
    }
    fn block(&mut self, block: &TlsBlock) -> Result<(), TransportError> {
        match block {
            TlsBlock::TlsBlockString(d) => self.append(&d.data),
            TlsBlock::TlsBlockZero(d) => self.zeros(
                usize::try_from(d.length)
                    .map_err(|_| TransportError::Protocol("bad TLS zero length"))?,
            ),
            TlsBlock::TlsBlockRandom(d) => {
                let n = usize::try_from(d.length)
                    .map_err(|_| TransportError::Protocol("bad TLS random length"))?;
                if n > 2048 {
                    return Err(TransportError::Protocol("bad TLS random length"));
                }
                let mut b = vec![0; n];
                self.entropy.fill(&mut b);
                self.append(&b)
            }
            TlsBlock::TlsBlockGrease(d) => {
                let Some(&g) = self.greases.get(d.seed as usize) else {
                    return Err(TransportError::Protocol("bad TLS grease seed"));
                };
                self.append(&[g; 2])
            }
            TlsBlock::TlsBlockDomain(_) => self.append(self.domain),
            TlsBlock::TlsBlockPublicKey(_) => {
                let key = self.entropy.public_key()?;
                self.append(&key)
            }
            TlsBlock::TlsBlockScope(d) => {
                let offset = self.result.len();
                self.append(&[0; 2])?;
                self.blocks(&d.entries)?;
                let size = self.result.len() - offset - 2;
                self.result[offset..offset + 2].copy_from_slice(&(size as u16).to_be_bytes());
                Ok(())
            }
            TlsBlock::TlsBlockPermutation(d) => {
                let mut parts = Vec::new();
                for blocks in &d.entries {
                    let mut part = Part {
                        domain: self.domain,
                        greases: self.greases,
                        entropy: self.entropy,
                        result: Vec::new(),
                        digest_position: None,
                    };
                    part.blocks(blocks)?;
                    parts.push(part.result);
                }
                let order = self.entropy.permutation(parts.len());
                let mut seen = vec![false; parts.len()];
                if order.len() != parts.len() {
                    return Err(TransportError::Protocol("invalid TLS permutation"));
                }
                for i in order {
                    if i >= parts.len() || seen[i] {
                        return Err(TransportError::Protocol("invalid TLS permutation"));
                    }
                    seen[i] = true;
                    self.append(&parts[i])?;
                }
                Ok(())
            }
            TlsBlock::TlsBlockM(_) => {
                let mut noise = vec![0; 384 * 8 + 32];
                self.entropy.fill(&mut noise);
                let mut bytes = Vec::with_capacity(1184);
                for pair in noise[..384 * 8].chunks_exact(8) {
                    let a = u32::from_le_bytes(pair[..4].try_into().unwrap()) % 3329;
                    let b = u32::from_le_bytes(pair[4..].try_into().unwrap()) % 3329;
                    bytes.extend([a as u8, ((a >> 8) + ((b & 15) << 4)) as u8, (b >> 4) as u8]);
                }
                let mut extra = [0; 32];
                self.entropy.fill(&mut extra);
                bytes.extend(extra);
                self.append(&bytes)
            }
            TlsBlock::TlsBlockE(_) => {
                let index = loop {
                    let n = self.entropy.u32();
                    let max = u32::MAX / 4 * 4;
                    if n < max {
                        break n % 4;
                    }
                };
                let size = [144, 176, 208, 240][index as usize];
                let mut bytes = vec![0; size];
                self.entropy.fill(&mut bytes);
                self.append(&bytes)
            }
            TlsBlock::TlsBlockPadding(_) => {
                if self.result.len() < 513 {
                    let size = 513 - self.result.len();
                    self.append(&[0, 0x15])?;
                    self.append(&(size as u16).to_be_bytes())?;
                    self.zeros(size)?;
                }
                Ok(())
            }
        }
    }
}
pub fn prepare_client_hello(
    domain: &[u8],
    key: &[u8],
    timestamp: i32,
    entropy: &mut impl HelloEntropy,
) -> Result<ClientHello, TransportError> {
    let mut greases = [0; 8];
    entropy.fill(&mut greases);
    for g in &mut greases {
        *g = (*g & 0xf0) + 0x0a;
    }
    for i in (0..8).step_by(2) {
        if greases[i] == greases[i + 1] {
            greases[i + 1] ^= 0x10;
        }
    }
    let mut part = Part {
        domain,
        greases: &greases,
        entropy,
        result: Vec::new(),
        digest_position: None,
    };
    part.blocks(&super::tls_rules::prepare_client_hello_rules())?;
    let offset = part
        .digest_position
        .ok_or(TransportError::Protocol("missing ClientHello digest"))?;
    let hash = tg_base::openssl_help::hmac_sha256(key, &part.result);
    part.result[offset..offset + 32].copy_from_slice(&hash);
    for (a, b) in part.result[offset + 28..offset + 32]
        .iter_mut()
        .zip(timestamp.to_le_bytes())
    {
        *a ^= b;
    }
    let digest = part.result[offset..offset + 32].try_into().unwrap();
    Ok(ClientHello {
        data: part.result,
        digest,
    })
}
/// Returns full hello length when available; verifies both headers and HMAC over
/// client digest + server reply with the server digest zeroed.
pub fn check_server_hello(
    bytes: &[u8],
    client_digest: &[u8; 32],
    key: &[u8],
) -> Result<Option<usize>, TransportError> {
    let bad = |s| TransportError::Protocol(s);
    if bytes.len() < 5 {
        return Ok(None);
    }
    if bytes[..3] != [0x16, 3, 3] {
        return Err(bad("Bad Server Hello part1."));
    }
    let p2 = u16::from_be_bytes(bytes[3..5].try_into().unwrap()) as usize;
    let p3 = 5 + p2;
    let p4 = p3 + 9 + 2;
    if p4 > 65536 {
        return Err(bad("Bad Server Hello size."));
    }
    if bytes.len() < p4 {
        return Ok(None);
    }
    if bytes[p3..p3 + 9] != [0x14, 3, 3, 0, 1, 1, 0x17, 3, 3] {
        return Err(bad("Bad Server Hello part."));
    }
    let size = p4 + u16::from_be_bytes(bytes[p4 - 2..p4].try_into().unwrap()) as usize;
    if !(43..=65536).contains(&size) {
        return Err(bad("Bad Server Hello size."));
    }
    if bytes.len() < size {
        return Ok(None);
    }
    let mut full = client_digest.to_vec();
    full.extend_from_slice(&bytes[..size]);
    let digest = full[43..75].to_vec();
    full[43..75].fill(0);
    let hash = tg_base::openssl_help::hmac_sha256(key, &full);
    let difference = digest
        .iter()
        .zip(hash)
        .fold(0u8, |a, (&b, c)| a | std::hint::black_box(b ^ c));
    if difference != 0 {
        return Err(bad("Bad Server Hello digest."));
    }
    Ok(Some(size))
}
pub fn frame(prefix: &[u8], mut buffer: &[u8]) -> Result<Vec<u8>, TransportError> {
    if buffer.is_empty() || prefix.len() >= 2878 {
        return Err(TransportError::Protocol("bad TLS outgoing payload"));
    }
    let mut out = Vec::new();
    let mut prefix = prefix;
    if !prefix.is_empty() {
        out.extend([0x14, 3, 3, 0, 1, 1]);
    }
    while !buffer.is_empty() {
        let size = buffer.len().min(2878 - prefix.len());
        out.extend([0x17, 3, 3]);
        out.extend(((prefix.len() + size) as u16).to_be_bytes());
        out.extend_from_slice(prefix);
        prefix = &[];
        out.extend_from_slice(&buffer[..size]);
        buffer = &buffer[size..];
    }
    Ok(out)
}
#[derive(Default)]
pub struct Records {
    buffer: Vec<u8>,
}
impl Records {
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<u8>, TransportError> {
        self.buffer.extend_from_slice(bytes);
        let mut result = Vec::new();
        loop {
            if self.buffer.len() <= 5 {
                break;
            }
            if self.buffer[..3] != [0x17, 3, 3] {
                return Err(TransportError::Protocol("Bad packet header."));
            }
            let size = u16::from_be_bytes(self.buffer[3..5].try_into().unwrap()) as usize;
            if self.buffer.len() < 5 + size {
                break;
            }
            result.extend_from_slice(&self.buffer[5..5 + size]);
            self.buffer.drain(..5 + size);
        }
        Ok(result)
    }
}

/// Tokio adapter for TcpConnection. Its first write contains the 64-byte
/// obfuscation prefix and a complete transport frame, as in TlsSocket::write.
pub struct TlsSocket<S> {
    stream: S,
    records: Records,
    incoming: std::collections::VecDeque<u8>,
    outgoing: Vec<u8>,
    written: usize,
    input_size: usize,
    first_write: bool,
}
impl<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin> TlsSocket<S> {
    pub async fn connect(
        mut stream: S,
        secret: &[u8],
        timestamp: i32,
        entropy: &mut impl HelloEntropy,
    ) -> Result<Self, TransportError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        if secret.len() < 21 || secret[0] != 0xee {
            return Err(TransportError::Protocol("invalid fake TLS secret"));
        }
        let hello = prepare_client_hello(&secret[17..], &secret[1..17], timestamp, entropy)?;
        stream.write_all(&hello.data).await?;
        let mut bytes = Vec::new();
        let full = loop {
            if let Some(size) = check_server_hello(&bytes, &hello.digest, &secret[1..17])? {
                break size;
            }
            let mut chunk = [0; 8192];
            let count = stream.read(&mut chunk).await?;
            if count == 0 {
                return Err(TransportError::Protocol(
                    "peer closed during fake TLS hello",
                ));
            }
            bytes.extend_from_slice(&chunk[..count]);
        };
        let mut records = Records::default();
        let incoming = records.feed(&bytes[full..])?.into();
        Ok(Self {
            stream,
            records,
            incoming,
            outgoing: Vec::new(),
            written: 0,
            input_size: 0,
            first_write: true,
        })
    }
    fn poll_output(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        use std::{pin::Pin, task::Poll};
        while self.written < self.outgoing.len() {
            let count = std::task::ready!(
                Pin::new(&mut self.stream).poll_write(cx, &self.outgoing[self.written..])
            )?;
            if count == 0 {
                return Poll::Ready(Err(std::io::ErrorKind::WriteZero.into()));
            }
            self.written += count;
        }
        self.outgoing.clear();
        self.written = 0;
        Poll::Ready(Ok(()))
    }
}
impl<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin> tokio::io::AsyncRead
    for TlsSocket<S>
{
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        to: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        use std::{pin::Pin, task::Poll};
        let this = self.get_mut();
        if to.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        loop {
            if !this.incoming.is_empty() {
                let count = to.remaining().min(this.incoming.len());
                for _ in 0..count {
                    to.put_slice(&[this.incoming.pop_front().unwrap()]);
                }
                return Poll::Ready(Ok(()));
            }
            let mut bytes = [0; 8192];
            let mut buffer = tokio::io::ReadBuf::new(&mut bytes);
            std::task::ready!(Pin::new(&mut this.stream).poll_read(cx, &mut buffer))?;
            if buffer.filled().is_empty() {
                return Poll::Ready(Ok(()));
            }
            match this.records.feed(buffer.filled()) {
                Ok(bytes) => this.incoming.extend(bytes),
                Err(e) => {
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        e.to_string(),
                    )))
                }
            }
        }
    }
}
impl<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin> tokio::io::AsyncWrite
    for TlsSocket<S>
{
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bytes: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        if this.outgoing.is_empty() {
            if bytes.is_empty() {
                return std::task::Poll::Ready(Ok(0));
            }
            let (prefix, body) = if this.first_write {
                if bytes.len() <= 64 {
                    return std::task::Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "first fake TLS write needs prefix and payload",
                    )));
                }
                bytes.split_at(64)
            } else {
                ([].as_slice(), bytes)
            };
            this.outgoing = frame(prefix, body).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string())
            })?;
            this.first_write = false;
            this.input_size = bytes.len();
        }
        std::task::ready!(this.poll_output(cx))?;
        std::task::Poll::Ready(Ok(this.input_size))
    }
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        std::task::ready!(this.poll_output(cx))?;
        std::pin::Pin::new(&mut this.stream).poll_flush(cx)
    }
    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        std::task::ready!(this.poll_output(cx))?;
        std::pin::Pin::new(&mut this.stream).poll_shutdown(cx)
    }
}
