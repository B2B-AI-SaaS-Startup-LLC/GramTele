use std::sync::{Arc, Mutex};
use tg_mtproto::connection_http::{
    encode_request, read_response, ConnectFuture, HttpConnection, HttpConnector, HttpHeaders,
    HttpStream,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
#[test]
fn post_matches_qt_capture() {
    let fixture = include_bytes!("fixtures/http-post.bin");
    let size = u32::from_le_bytes(fixture[..4].try_into().unwrap()) as usize;
    let body = tg_tl::primes_to_bytes(&[1, 2, 3]);
    assert_eq!(
        encode_request("example.com", &body, &HttpHeaders::default()).unwrap(),
        fixture[4..4 + size]
    );
    assert!(encode_request("host\r\nInjected: true", &body, &HttpHeaders::default()).is_err());
}
#[tokio::test]
async fn status_spacing_chunking_interim_and_eof() {
    for (wire,expected,keep) in [
  (b"HTTP/1.0  200 OK\r\nContent-Length: 8\r\n\r\n12345678".as_slice(),b"12345678".as_slice(),false),
  (b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3;foo=bar\r\nabc\r\n2\r\nde\r\n0\r\nX-Trailer: ignored\r\n\r\n",b"abcde",true),
  (b"HTTP/1.0 200 OK\r\n\r\nabc",b"abc",false),
 ] {let mut reader=BufReader::new(wire);let response=read_response(&mut reader).await.unwrap();assert_eq!(response.status,200);assert_eq!(response.body,expected);assert_eq!(response.keep_alive,keep);}
}
#[tokio::test]
async fn rejects_conflicting_lengths_truncation_and_oversize() {
    for wire in [
        b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\na".as_slice(),
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\na",
        b"HTTP/1.1 200 OK\r\nContent-Length: 67108865\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nxBAD\r\n",
        b"HTTP/1.1 xyz OK\r\n\r\n",
    ] {
        assert!(read_response(&mut BufReader::new(wire)).await.is_err());
    }
}
struct Connector(Mutex<Option<tokio::io::DuplexStream>>);
impl HttpConnector for Connector {
    fn connect(&self) -> ConnectFuture<'_> {
        Box::pin(async {
            Ok(Box::new(
                self.0
                    .lock()
                    .unwrap()
                    .take()
                    .expect("keep-alive must reuse connection"),
            ) as HttpStream)
        })
    }
}
#[tokio::test]
async fn keep_alive_two_requests_and_disconnect() {
    let (client, mut server) = tokio::io::duplex(128);
    let connection = HttpConnection::from_connector(
        "example.com".into(),
        Arc::new(Connector(Mutex::new(Some(client)))),
        HttpHeaders::default(),
    );
    let peer = async move {
        for _ in 0..2 {
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"\r\n\r\n") {
                bytes.push(server.read_u8().await.unwrap());
            }
            let mut body = [0; 8];
            server.read_exact(&mut body).await.unwrap();
            assert_eq!(body, [1, 0, 0, 0, 2, 0, 0, 0]);
            server.write_all(b"HTTP/1.0  200 OK\r\nConnection: keep-alive\r\nContent-Length: 8\r\n\r\n\x03\0\0\0\x04\0\0\0").await.unwrap();
        }
    };
    let caller = async {
        assert!(connection.need_http_wait());
        for _ in 0..2 {
            assert_eq!(
                connection.send_data(vec![0, 0, 1, 2]).await.unwrap(),
                [3, 4]
            );
            assert!(connection.need_http_wait());
        }
        connection.disconnect_from_server();
        assert!(!connection.need_http_wait());
        assert!(connection.send_data(vec![0, 0, 1, 2]).await.is_err());
    };
    tokio::join!(peer, caller);
}
#[tokio::test]
async fn disconnect_cancels_pending_response() {
    let (client, mut server) = tokio::io::duplex(4096);
    let connection = HttpConnection::from_connector(
        "example.com".into(),
        Arc::new(Connector(Mutex::new(Some(client)))),
        HttpHeaders::default(),
    );
    let request = connection.send_data(vec![0, 0, 1, 2]);
    let cancel = async {
        let mut byte = [0];
        server.read_exact(&mut byte).await.unwrap();
        assert!(!connection.need_http_wait());
        connection.disconnect_from_server();
    };
    let (result, ()) = tokio::join!(request, cancel);
    assert!(result.is_err());
}
