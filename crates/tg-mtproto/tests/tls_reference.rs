use tg_mtproto::{
    connection_abstract::TransportError,
    core_types::Random,
    details::mtproto_tls_socket::{
        check_server_hello, frame, prepare_client_hello, HelloEntropy, Records,
    },
};
struct Entropy {
    counter: u8,
    public: [u8; 32],
}
impl Random for Entropy {
    fn fill(&mut self, data: &mut [u8]) {
        for b in data {
            *b = self.counter;
            self.counter = self.counter.wrapping_add(1);
        }
    }
}
impl HelloEntropy for Entropy {
    fn public_key(&mut self) -> Result<[u8; 32], TransportError> {
        Ok(self.public)
    }
    fn permutation(&mut self, length: usize) -> Vec<usize> {
        (0..length).rev().collect()
    }
}
#[test]
fn original_client_hello_all_greases_extensions_lengths_and_hmac() {
    let fixture = include_bytes!("fixtures/tls.bin");
    let public = fixture[..32].try_into().unwrap();
    let size = u32::from_le_bytes(fixture[32..36].try_into().unwrap()) as usize;
    let mut entropy = Entropy { counter: 1, public };
    let key: Vec<_> = (0..16).collect();
    let hello = prepare_client_hello(b"example.com", &key, 1700000000, &mut entropy).unwrap();
    assert_eq!(hello.data, fixture[36..36 + size]);
    assert_eq!(hello.digest, fixture[36 + size..]);
}
#[test]
fn server_hello_fragmentation_and_hmac_rejection() {
    let secret = [3; 16];
    let client_digest = [7; 32];
    let mut server = vec![0x16, 3, 3, 0, 43];
    server.resize(48, 0);
    server.extend([0x14, 3, 3, 0, 1, 1, 0x17, 3, 3, 0, 4]);
    server.extend([1, 2, 3, 4]);
    let mut payload = client_digest.to_vec();
    payload.extend_from_slice(&server);
    let digest = tg_base::openssl_help::hmac_sha256(&secret, &payload);
    server[11..43].copy_from_slice(&digest);
    for n in 0..server.len() {
        assert_eq!(
            check_server_hello(&server[..n], &client_digest, &secret).unwrap(),
            None
        );
    }
    assert_eq!(
        check_server_hello(&server, &client_digest, &secret).unwrap(),
        Some(server.len())
    );
    server[11] ^= 1;
    assert!(check_server_hello(&server, &client_digest, &secret).is_err());
    server[0] = 0;
    assert!(check_server_hello(&server, &client_digest, &secret).is_err());
}
#[test]
fn record_chunks_and_every_byte_fragment() {
    let body = vec![42; 9000];
    let prefix = [17; 64];
    let bytes = frame(&prefix, &body).unwrap();
    assert_eq!(&bytes[..6], &[0x14, 3, 3, 0, 1, 1]);
    let mut decoder = Records::default();
    let mut output = Vec::new();
    for byte in &bytes[6..] {
        output.extend(decoder.feed(&[*byte]).unwrap());
    }
    let mut expected = prefix.to_vec();
    expected.extend(body);
    assert_eq!(output, expected);
    assert!(decoder.feed(&[0, 0, 0, 0, 0, 1]).is_err());
}
#[tokio::test(flavor = "current_thread")]
async fn tls_adapter_on_duplex_stream() {
    use tg_mtproto::details::mtproto_tls_socket::TlsSocket;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let fixture = include_bytes!("fixtures/tls.bin");
    let public = fixture[..32].try_into().unwrap();
    let size = u32::from_le_bytes(fixture[32..36].try_into().unwrap()) as usize;
    let (client, mut server) = tokio::io::duplex(128);
    let peer = async move {
        let mut hello = vec![0; size];
        server.read_exact(&mut hello).await.unwrap();
        assert_eq!(hello, fixture[36..36 + size]);
        let digest = &hello[11..43];
        let key: Vec<_> = (0..16).collect();
        let mut reply = vec![0x16, 3, 3, 0, 43];
        reply.resize(48, 0);
        reply.extend([0x14, 3, 3, 0, 1, 1, 0x17, 3, 3, 0, 4, 1, 2, 3, 4]);
        let mut input = digest.to_vec();
        input.extend_from_slice(&reply);
        let hash = tg_base::openssl_help::hmac_sha256(&key, &input);
        reply[11..43].copy_from_slice(&hash);
        server.write_all(&reply).await.unwrap();
        let expected = frame(&[9; 64], &[8; 32]).unwrap();
        let mut actual = vec![0; expected.len()];
        server.read_exact(&mut actual).await.unwrap();
        assert_eq!(actual, expected);
        server
            .write_all(&[0x17, 3, 3, 0, 3, 1, 2, 3])
            .await
            .unwrap();
    };
    let caller = async move {
        let mut entropy = Entropy { counter: 1, public };
        let mut secret = vec![0xee];
        secret.extend(0..16);
        secret.extend_from_slice(b"example.com");
        let mut socket = TlsSocket::connect(client, &secret, 1700000000, &mut entropy)
            .await
            .unwrap();
        let bytes = [vec![9; 64], vec![8; 32]].concat();
        socket.write_all(&bytes).await.unwrap();
        let mut reply = [0; 3];
        socket.read_exact(&mut reply).await.unwrap();
        assert_eq!(reply, [1, 2, 3]);
    };
    tokio::join!(caller, peer);
}
#[test]
fn x25519_public_key_matches_original_openssl_keygen() {
    let private = std::array::from_fn(|i| i as u8 + 1);
    assert_eq!(
        tg_mtproto::details::mtproto_tls_socket::x25519_public_key(private).as_slice(),
        &include_bytes!("fixtures/tls.bin")[..32]
    );
}
