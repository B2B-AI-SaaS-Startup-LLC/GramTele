use tg_mtproto::{
    connection_tcp::{is_good_start_nonce, Protocol, TcpCodec},
    core_types::Random,
};
struct Counter(u8);
impl Random for Counter {
    fn fill(&mut self, data: &mut [u8]) {
        for b in data {
            *b = self.0;
            self.0 = self.0.wrapping_add(1);
        }
    }
}
#[test]
fn original_obfuscated_headers_and_first_frames() {
    let mut fixture = include_bytes!("fixtures/tcp.bin").as_slice();
    for protocol in [
        Protocol::Version0,
        Protocol::Version1([0x11; 16]),
        Protocol::VersionD([0x11; 16]),
    ] {
        let size = u32::from_le_bytes(fixture[..4].try_into().unwrap()) as usize;
        let expected = &fixture[4..4 + size];
        fixture = &fixture[4 + size..];
        let mut random = Counter(1);
        let (header, mut codec) = TcpCodec::start(protocol, 10002, &mut random);
        assert_eq!(header, expected[..64]);
        let frame = codec
            .encode(
                &tg_tl::primes_to_bytes(&[0x12345678, -1, 0x07060504]),
                &mut random,
            )
            .unwrap();
        assert_eq!(frame, expected[64..]);
    }
    assert!(fixture.is_empty());
}
#[test]
fn forbidden_first_bytes_and_zero_second_word() {
    let mut nonce = [1; 64];
    assert!(is_good_start_nonce(&nonce));
    for word in [
        0x44414548u32,
        0x54534f50,
        0x20544547,
        0xeeeeeeee,
        0xdddddddd,
        0x02010316,
    ] {
        nonce[..4].copy_from_slice(&word.to_le_bytes());
        assert!(!is_good_start_nonce(&nonce));
    }
    nonce = [1; 64];
    nonce[0] = 0xef;
    assert!(!is_good_start_nonce(&nonce));
    nonce = [1; 64];
    nonce[4..8].fill(0);
    assert!(!is_good_start_nonce(&nonce));
}
#[test]
fn framing_boundaries_and_fragment_headers() {
    let p = Protocol::Version0;
    let mut random = Counter(0);
    for count in [1, 126, 127, 128, 4096] {
        let body = vec![0x44; count * 4];
        let frame = p.frame(&body, &mut random).unwrap();
        let prefix = if count < 127 { 1 } else { 4 };
        assert_eq!(
            p.packet_length(&frame).unwrap(),
            Some((body.len() + prefix, prefix))
        );
        assert_eq!(&frame[prefix..], body);
        for n in 0..prefix {
            assert_eq!(p.packet_length(&frame[..n]).unwrap(), None);
        }
    }
    for bad in [&[0u8][..], &[128][..], &[127, 1, 0, 0][..]] {
        assert!(p.packet_length(bad).is_err());
    }
    let d = Protocol::VersionD([1; 16]);
    assert!(d.packet_length(&[0; 4]).is_err());
    assert!(d.packet_length(&u32::MAX.to_le_bytes()).is_err());
    assert_eq!(d.packet_length(&[8, 0, 0]).unwrap(), None);
}

#[tokio::test(flavor = "current_thread")]
async fn tcp_duplex_all_protocols_fragmented_reads() {
    use tg_mtproto::{connection_tcp::TcpConnection, mtproto_auth_key::CtrState};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for protocol in [
        Protocol::Version0,
        Protocol::Version1([0x11; 16]),
        Protocol::VersionD([0x11; 16]),
    ] {
        let (client, mut server) = tokio::io::duplex(128);
        let p = protocol.clone();
        let peer = async move {
            let mut header = [0; 64];
            server.read_exact(&mut header).await.unwrap();
            let derive = |source: &[u8]| -> [u8; 32] {
                match &p {
                    Protocol::Version0 => source.try_into().unwrap(),
                    Protocol::Version1(s) | Protocol::VersionD(s) => {
                        tg_base::openssl_help::sha256_parts(&[source, s])
                            .try_into()
                            .unwrap()
                    }
                }
            };
            let mut receive =
                CtrState::new(&derive(&header[8..40]), &header[40..56].try_into().unwrap());
            let mut reverse = header[8..56].to_vec();
            reverse.reverse();
            let mut send =
                CtrState::new(&derive(&reverse[..32]), &reverse[32..].try_into().unwrap());
            let mut nonce = header;
            receive.apply(&mut nonce);
            assert_eq!(nonce[56..60], p.id().to_le_bytes());
            let mut received = Vec::new();
            let (size, prefix) = loop {
                let mut byte = [server.read_u8().await.unwrap()];
                receive.apply(&mut byte);
                received.extend(byte);
                if let Some(size) = p.packet_length(&received).unwrap() {
                    break size;
                }
            };
            let mut rest = vec![0; size - received.len()];
            server.read_exact(&mut rest).await.unwrap();
            receive.apply(&mut rest);
            received.extend(rest);
            assert_eq!(
                tg_tl::bytes_to_primes(&received[prefix..prefix + 12]),
                vec![1, 2, 3]
            );
            let mut reply = p
                .frame(&tg_tl::primes_to_bytes(&[9, 8, 7]), &mut Counter(3))
                .unwrap();
            send.apply(&mut reply);
            for byte in reply {
                server.write_all(&[byte]).await.unwrap();
            }
        };
        let caller = async move {
            let connection = TcpConnection::new(client, protocol, 10002, Counter(1))
                .await
                .unwrap();
            let (mut reader, mut writer) = connection.into_split();
            let (sent, received) =
                tokio::join!(writer.send_data(vec![0, 0, 1, 2, 3]), reader.receive(),);
            sent.unwrap();
            assert_eq!(received.unwrap(), vec![9, 8, 7]);
            assert!(!reader.state.events.is_empty());
        };
        tokio::join!(caller, peer);
    }
}
