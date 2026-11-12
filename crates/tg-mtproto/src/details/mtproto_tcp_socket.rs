//! Tokio socket adapter and proxy negotiations, independent of MTProto framing.
pub use crate::connection_tcp::is_good_start_nonce;
use crate::{
    connection_abstract::TransportError,
    mtproto_proxy_data::{ProxyData, Type},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
pub fn socks_greeting(auth: bool) -> Vec<u8> {
    if auth {
        vec![5, 1, 2]
    } else {
        vec![5, 1, 0]
    }
}
pub fn socks_credentials(user: &str, password: &str) -> Result<Vec<u8>, TransportError> {
    let user = latin1(user);
    let password = latin1(password);
    if user.len() > 255 || password.len() > 255 {
        return Err(TransportError::Protocol(
            "SOCKS5 credentials exceed 255 bytes",
        ));
    }
    let mut out = vec![1, user.len() as u8];
    out.extend(user);
    out.push(password.len() as u8);
    out.extend(password);
    Ok(out)
}
fn latin1(value: &str) -> Vec<u8> {
    value
        .chars()
        .map(|c| if c as u32 <= 255 { c as u8 } else { b'?' })
        .collect()
}
pub fn socks_connect_request(host: &str, port: u16) -> Result<Vec<u8>, TransportError> {
    let mut out = vec![5, 1, 0];
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            out.push(1);
            out.extend(ip.octets());
        }
        Ok(std::net::IpAddr::V6(ip)) => {
            out.push(4);
            out.extend(ip.octets());
        }
        Err(_) => {
            let host = reqwest::Url::parse(&format!("https://{host}"))
                .ok()
                .and_then(|u| u.domain().map(str::to_owned))
                .ok_or(TransportError::Protocol("invalid SOCKS5 hostname"))?;
            if host.len() > 255 {
                return Err(TransportError::Protocol(
                    "SOCKS5 hostname exceeds 255 bytes",
                ));
            }
            out.extend([3, host.len() as u8]);
            out.extend_from_slice(host.as_bytes());
        }
    }
    out.extend(port.to_be_bytes());
    Ok(out)
}
pub async fn socks_handshake<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    host: &str,
    port: u16,
    user: &str,
    password: &str,
) -> Result<(), TransportError> {
    let auth = !user.is_empty() || !password.is_empty();
    stream.write_all(&socks_greeting(auth)).await?;
    let mut response = [0; 2];
    stream.read_exact(&mut response).await?;
    if response[0] != 5 {
        return Err(TransportError::Protocol("bad SOCKS5 greeting version"));
    }
    match response[1] {
        0 => {}
        2 => {
            stream
                .write_all(&socks_credentials(user, password)?)
                .await?;
            stream.read_exact(&mut response).await?;
            if response != [1, 0] {
                return Err(TransportError::Protocol("SOCKS5 authentication failed"));
            }
        }
        _ => {
            return Err(TransportError::Protocol(
                "SOCKS5 has no acceptable authentication",
            ))
        }
    };
    stream
        .write_all(&socks_connect_request(host, port)?)
        .await?;
    let mut header = [0; 4];
    stream.read_exact(&mut header).await?;
    if header[0] != 5 || header[1] != 0 || header[2] != 0 {
        return Err(TransportError::Protocol("SOCKS5 CONNECT rejected"));
    }
    let size = match header[3] {
        1 => 4,
        4 => 16,
        3 => stream.read_u8().await? as usize,
        _ => return Err(TransportError::Protocol("bad SOCKS5 bound address type")),
    };
    let mut tail = vec![0; size + 2];
    stream.read_exact(&mut tail).await?;
    Ok(())
}
pub fn http_connect_request(host: &str, port: u16, user: &str, password: &str) -> Vec<u8> {
    let host = if host.parse::<std::net::Ipv6Addr>().is_ok() {
        format!("[{host}]")
    } else {
        host.into()
    };
    let mut text = format!(
        "CONNECT {host}:{port} HTTP/1.1\r\nProxy-Connection: keep-alive\r\nHost: {host}\r\nUser-Agent: Mozilla/5.0\r\n"
    );
    if !user.is_empty() || !password.is_empty() {
        let encoded = super::mtproto_rsa_public_key::base64_url_encode(
            format!("{user}:{password}").as_bytes(),
        )
        .replace('-', "+")
        .replace('_', "/");
        let padding = "=".repeat((4 - encoded.len() % 4) % 4);
        text += &format!("Proxy-Authorization: Basic {encoded}{padding}\r\n");
    }
    text.push_str("\r\n");
    text.into_bytes()
}
pub async fn http_connect_handshake<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    host: &str,
    port: u16,
    user: &str,
    password: &str,
) -> Result<(), TransportError> {
    for authenticated in [false, true] {
        stream
            .write_all(&http_connect_request(
                host,
                port,
                if authenticated { user } else { "" },
                if authenticated { password } else { "" },
            ))
            .await?;
        let mut response = Vec::new();
        loop {
            if response.len() > 65536 {
                return Err(TransportError::Protocol(
                    "HTTP CONNECT header exceeds 64 KiB",
                ));
            }
            response.push(stream.read_u8().await?);
            if response.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        let text = String::from_utf8_lossy(&response);
        let mut lines = text.split("\r\n");
        let first = lines.next().unwrap_or_default();
        let status = first.split_whitespace().nth(1).unwrap_or_default();
        if status == "200" {
            return Ok(());
        }
        if status != "407" || authenticated || user.is_empty() && password.is_empty() {
            return Err(TransportError::Protocol("HTTP CONNECT rejected"));
        }
        let headers: Vec<_> = lines.filter_map(|l| l.split_once(':')).collect();
        if !headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("proxy-authenticate")
                && value
                    .trim_start()
                    .get(..5)
                    .is_some_and(|v| v.eq_ignore_ascii_case("basic"))
        }) {
            return Err(TransportError::Protocol(
                "HTTP CONNECT authentication requires unsupported Digest/NTLM/Negotiate",
            ));
        }
        let length = headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .map_or(Ok(0usize), |(_, value)| value.trim().parse())
            .map_err(|_| TransportError::Protocol("bad CONNECT content length"))?;
        if length > 65536 {
            return Err(TransportError::Protocol(
                "CONNECT challenge body exceeds 64 KiB",
            ));
        }
        let mut body = vec![0; length];
        stream.read_exact(&mut body).await?;
    }
    Err(TransportError::Protocol(
        "HTTP CONNECT authentication failed",
    ))
}

pub async fn connect_to_host(
    address: &str,
    port: u16,
    proxy: &ProxyData,
    for_files: bool,
) -> Result<tokio::net::TcpStream, TransportError> {
    let proxied = matches!(proxy.kind, Type::Socks5 | Type::Http);
    let (host, socket_port) = if proxied {
        (
            proxy.host.as_str(),
            u16::try_from(proxy.port)
                .map_err(|_| TransportError::Protocol("proxy port exceeds 65535"))?,
        )
    } else {
        (address, port)
    };
    let mut stream = if for_files {
        let mut result = None;
        let mut last_error = None;
        for addr in tokio::net::lookup_host((host, socket_port)).await? {
            let socket = if addr.is_ipv6() {
                tokio::net::TcpSocket::new_v6()?
            } else {
                tokio::net::TcpSocket::new_v4()?
            };
            let _ =
                socket.set_send_buffer_size(super::mtproto_abstract_socket::FILES_SEND_BUFFER_SIZE);
            let _ = socket
                .set_recv_buffer_size(super::mtproto_abstract_socket::FILES_RECEIVE_BUFFER_SIZE);
            match socket.connect(addr).await {
                Ok(stream) => {
                    result = Some(stream);
                    break;
                }
                Err(error) => last_error = Some(error),
            }
        }
        result.ok_or_else(|| {
            TransportError::Io(
                last_error.unwrap_or_else(|| std::io::Error::other("no socket addresses")),
            )
        })?
    } else {
        tokio::net::TcpStream::connect((host, socket_port)).await?
    };
    match proxy.kind {
        Type::Socks5 => {
            socks_handshake(&mut stream, address, port, &proxy.user, &proxy.password).await?
        }
        Type::Http => {
            http_connect_handshake(&mut stream, address, port, &proxy.user, &proxy.password).await?
        }
        _ => {}
    }
    Ok(stream)
}
