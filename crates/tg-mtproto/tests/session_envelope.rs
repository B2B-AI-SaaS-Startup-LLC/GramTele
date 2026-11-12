use tg_mtproto::{
    mtproto_auth_key::AuthKey,
    session_private::{decrypt_message, encrypt_message, Direction, Message},
};
const SESSION: u64 = 0x8877665544332211;
fn key() -> AuthKey {
    AuthKey::local(std::array::from_fn(|i| i as u8))
}
fn message() -> Message {
    Message {
        salt: 0x1122334455667788,
        session_id: SESSION,
        msg_id: (1700000000u64 << 32) | 4,
        seq_no: 1,
        body: vec![0xc4f9186bu32 as i32],
    }
}
fn fixtures() -> (&'static [u8], &'static [u8]) {
    let bytes = include_bytes!("fixtures/session-envelope.bin");
    let n = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    let m = u32::from_le_bytes(bytes[4 + n..8 + n].try_into().unwrap()) as usize;
    (&bytes[4..4 + n], &bytes[8 + n..8 + n + m])
}
#[test]
fn original_send_secure_request_with_fixed_entropy() {
    let (outgoing, _) = fixtures();
    let padding: Vec<_> = (1..=12).collect();
    let packet = encrypt_message(&key(), &message(), &padding, Direction::ClientToServer).unwrap();
    assert_eq!(tg_tl::primes_to_bytes(&packet), outgoing);
}
#[test]
fn original_auth_key_incoming_direction_and_tcp_padding() {
    let (_, incoming) = fixtures();
    let packet = tg_tl::bytes_to_primes(incoming);
    let mut expected = message();
    expected.msg_id = (1700000000u64 << 32) | 1;
    let result = decrypt_message(&key(), &packet[2..], SESSION, 1700000000).unwrap();
    assert_eq!(result.message, expected);
    assert!(!result.bad_time);
    assert_eq!(result.padding_len, 12);
    assert!(result.message.needs_ack());
    for extra in 0..4 {
        let mut padded = packet[2..].to_vec();
        padded.extend(vec![0x12345678; extra]);
        assert_eq!(
            decrypt_message(&key(), &padded, SESSION, 1700000000)
                .unwrap()
                .message,
            expected
        );
    }
    assert!(
        decrypt_message(&key(), &packet[2..], SESSION, 1700000301)
            .unwrap()
            .bad_time
    );
    assert!(
        !decrypt_message(&key(), &packet[2..], SESSION, 1700000300)
            .unwrap()
            .bad_time
    );
    assert!(
        decrypt_message(&key(), &packet[2..], SESSION, 1699999939)
            .unwrap()
            .bad_time
    );
    assert!(
        !decrypt_message(&key(), &packet[2..], SESSION, 1699999940)
            .unwrap()
            .bad_time
    );
}
#[test]
fn tampered_key_id_digest_ciphertext_and_session_are_rejected() {
    let (_, incoming) = fixtures();
    let packet = tg_tl::bytes_to_primes(incoming);
    assert!(decrypt_message(&key(), &packet[2..], SESSION ^ 1, 1700000000).is_err());
    for index in 2..packet.len() {
        let mut changed = packet.clone();
        changed[index] ^= 1;
        assert!(decrypt_message(&key(), &changed[2..], SESSION, 1700000000).is_err());
    }
    for n in 0..packet.len() - 3 {
        assert!(decrypt_message(&key(), &packet[2..n + 2], SESSION, 1700000000).is_err());
    }
}
// Independent server-side envelope construction: field layout and SHA256/AES
// key derivation follow the wire description, without calling client encoder or
// AuthKey::prepare_aes. The AES block primitive is the shared approved backend.
fn server_packet(msg: &Message, padding: usize, declared_length: u32) -> Vec<i32> {
    use aes::cipher::{BlockModeEncrypt, KeyIvInit};
    use tg_base::openssl_help::sha256_parts;
    let auth = std::array::from_fn::<_, 256, _>(|i| i as u8);
    let mut plain = Vec::new();
    for value in [msg.salt, msg.session_id, msg.msg_id] {
        plain.extend(value.to_le_bytes());
    }
    plain.extend(msg.seq_no.to_le_bytes());
    plain.extend(declared_length.to_le_bytes());
    plain.extend(tg_tl::primes_to_bytes(&msg.body));
    plain.resize(plain.len() + padding, 0xaa);
    let large = sha256_parts(&[&auth[96..128], &plain]);
    let mk = &large[8..24];
    let a = sha256_parts(&[mk, &auth[8..44]]);
    let b = sha256_parts(&[&auth[48..84], mk]);
    let aes: [u8; 32] = [&a[..8], &b[8..24], &a[24..32]]
        .concat()
        .try_into()
        .unwrap();
    let iv: [u8; 32] = [&b[..8], &a[8..24], &b[24..32]]
        .concat()
        .try_into()
        .unwrap();
    let mut cipher = ige::Encryptor::<aes::Aes256>::new((&aes).into(), (&iv).into());
    for block in plain.chunks_exact_mut(16) {
        cipher.encrypt_block(block.try_into().unwrap());
    }
    let bytes = [key().key_id().to_le_bytes().as_slice(), mk, &plain].concat();
    tg_tl::bytes_to_primes(&bytes)
}
#[test]
fn authenticated_invalid_lengths_padding_and_msg_ids() {
    let mut msg = message();
    msg.msg_id |= 1;
    for length in [1, 3, 8, u32::MAX, 16 * 1024 * 1024 + 4] {
        let packet = server_packet(&msg, 12, length);
        assert!(decrypt_message(&key(), &packet, SESSION, 1700000000).is_err());
    }
    for padding in [0, 4, 8, 1028] {
        let body_words = (16 - (32 + padding) % 16) % 16 / 4;
        msg.body = vec![1; body_words];
        let packet = server_packet(&msg, padding, (body_words * 4) as u32);
        assert!(decrypt_message(&key(), &packet, SESSION, 1700000000).is_err());
    }
    msg = message();
    for low in [0, 2] {
        msg.msg_id = (1700000000u64 << 32) | low;
        let packet = server_packet(&msg, 12, 4);
        assert!(decrypt_message(&key(), &packet, SESSION, 1700000000).is_err());
    }
    msg.msg_id |= 1;
    let packet = server_packet(&msg, 12, 4);
    assert!(decrypt_message(&key(), &packet, SESSION, 1700000000).is_ok());
}
