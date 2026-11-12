//! Only test DC 2, only the unauthenticated DH exchange. No API/account methods.
use tg_mtproto::{
    connection_abstract::{Clock, TransportError},
    connection_http::HttpConnection,
    connection_tcp::{Protocol, TcpConnection},
    core_types::{SystemRandom, Time},
    details::mtproto_dc_key_creator::create_key,
    mtproto_dc_options::{DcOptions, Environment},
};
struct LiveClock(std::time::Instant);
impl Clock for LiveClock {
    fn update_unix_time(&self, server_time: i32) {
        tg_base::unixtime::update(server_time, false);
    }
    fn next_message_id(&self) -> u64 {
        tg_base::unixtime::mtproto_msg_id()
    }
    fn now(&self) -> Time {
        Time {
            monotonic_ms: self.0.elapsed().as_millis() as i64,
            unix_seconds: tg_base::unixtime::now() as i64,
        }
    }
}
const TEST_DC_2: &str = "149.154.167.40";
#[tokio::test(flavor = "current_thread")]
#[ignore = "manual, test Telegram DC only: TCP unauthenticated DH"]
async fn authorization_key_test_dc_tcp() {
    let result = tokio::time::timeout(std::time::Duration::from_secs(40), async {
        let stream = tokio::net::TcpStream::connect((TEST_DC_2, 443))
            .await
            .map_err(TransportError::from)?;
        let mut connection =
            TcpConnection::new(stream, Protocol::Version0, 10002, SystemRandom).await?;
        let keys = DcOptions::new(Environment::Test).public_keys(2);
        let (key, _) = create_key(
            &mut connection,
            &LiveClock(std::time::Instant::now()),
            2,
            10002,
            0,
            &keys,
            &mut SystemRandom,
        )
        .await?;
        println!("TEST DC TCP: DH succeeded, keyId={:016x}", key.key_id());
        Ok::<_, Box<dyn std::error::Error>>(())
    })
    .await;
    println!("TCP result: {result:?}");
    assert!(matches!(result, Ok(Ok(()))));
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "manual, test Telegram DC only: HTTP unauthenticated DH"]
async fn authorization_key_test_dc_http() {
    let result = tokio::time::timeout(std::time::Duration::from_secs(40), async {
        let mut connection = HttpConnection::new(TEST_DC_2, Default::default());
        let keys = DcOptions::new(Environment::Test).public_keys(2);
        let (key, _) = create_key(
            &mut connection,
            &LiveClock(std::time::Instant::now()),
            2,
            10002,
            0,
            &keys,
            &mut SystemRandom,
        )
        .await?;
        println!("TEST DC HTTP: DH succeeded, keyId={:016x}", key.key_id());
        Ok::<_, Box<dyn std::error::Error>>(())
    })
    .await;
    if !matches!(&result, Ok(Ok(()))) {
        let diagnostic =
            tokio::time::timeout(std::time::Duration::from_secs(5), raw_http_probe()).await;
        println!("HTTP raw probe: {diagnostic:?}");
    }
    println!("HTTP result: {result:?}");
    assert!(matches!(result, Ok(Ok(()))));
}

async fn raw_http_probe() -> std::io::Result<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect((TEST_DC_2, 80)).await?;
    let packet = tg_mtproto::connection_abstract::prepare_not_secure_packet(
        &tg_scheme::ReqPqMulti::new(tg_tl::Int128::default()),
        tg_base::unixtime::mtproto_msg_id(),
        &mut SystemRandom,
    );
    let body = tg_mtproto::connection_http::request_body(&packet).unwrap();
    let header=format!("POST /api HTTP/1.1\r\nHost: {TEST_DC_2}\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(&body).await?;
    let mut bytes = [0; 256];
    let count = stream.read(&mut bytes).await?;
    Ok(format!(
        "first {count} bytes: {:?}; hex={:02x?}",
        String::from_utf8_lossy(&bytes[..count]),
        &bytes[..count]
    ))
}
