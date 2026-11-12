//! An independent server decrypts the client's RSA_PAD using its own private key.
use tg_base::openssl_help::{sha1, sha256, sha256_parts, BigNum};
use tg_mtproto::{
    core_types::{Random, Time},
    details::{
        mtproto_dc_key_creator::{
            encrypt_client_dh_inner, nonce_digest, temporary_aes, KeyExchange, Step,
        },
        mtproto_rsa_public_key::RsaPublicKey,
    },
    mtproto_auth_key::{aes_ige_decrypt_raw, AuthKey},
    mtproto_dh_utils::GOOD_PRIME,
};
use tg_scheme::*;
use tg_tl::{Deserialize, Int128, Reader};
struct Entropy(u64);
impl Random for Entropy {
    fn fill(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            *byte = self.0 as u8;
        }
    }
}
fn rsa() -> (RsaPublicKey, BigNum) {
    let mut bytes = include_bytes!("fixtures/test-server-rsa.bin").as_slice();
    let mut values = Vec::new();
    for _ in 0..3 {
        let size = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        values.push(bytes[4..4 + size].to_vec());
        bytes = &bytes[4 + size..];
    }
    (
        RsaPublicKey::from_components(&values[0], &values[1]).unwrap(),
        BigNum::from_bytes(&values[2]),
    )
}
fn decode<T: Deserialize>(bytes: &[i32]) -> T {
    T::read(&mut Reader::new(bytes)).unwrap()
}
fn send(step: Step) -> Vec<i32> {
    match step {
        Step::Send(b) => b,
        _ => panic!("expected outgoing request"),
    }
}
#[test]
fn memory_server_rsa_pad_dh_success_and_retry() {
    for retry in [false, true] {
        run_exchange(retry);
    }
}
fn run_exchange(retry: bool) {
    let (public, private) = rsa();
    let mut random = Entropy(0x123456789abcdef);
    let mut client = KeyExchange::new(2, 10002, 86400);
    let request = client.start(&mut random).unwrap();
    let pq: ReqPqMulti = decode(&request);
    let nonce = pq.nonce;
    let server_nonce = Int128 {
        l: 0x8877665544332211,
        h: 0x123456789abcdef,
    };
    let response = mtp_res_pq(
        nonce,
        server_nonce,
        323u16.to_be_bytes().to_vec(),
        vec![public.fingerprint()],
    );
    let request = send(
        client
            .receive(
                &tg_tl::serialize(&response),
                std::slice::from_ref(&public),
                Time::default(),
                &mut random,
            )
            .unwrap(),
    );
    let request: ReqDHParams = decode(&request);
    assert_eq!(request.p, 17u32.to_be_bytes());
    assert_eq!(request.q, 19u32.to_be_bytes());
    let decrypted = BigNum::mod_exp(
        &BigNum::from_bytes(&request.encrypted_data),
        &private,
        &BigNum::from_bytes(public.get_n()),
    );
    let mut padded = AuthKey::fill_data(&decrypted.get_bytes());
    let hash = sha256(&padded[32..]);
    let temp_key: [u8; 32] = std::array::from_fn(|i| padded[i] ^ hash[i]);
    aes_ige_decrypt_raw(&mut padded[32..], &temp_key, &[0; 32]).unwrap();
    let mut data = padded[32..224].to_vec();
    data.reverse();
    assert_eq!(
        padded[224..],
        sha256_parts(&[temp_key.as_slice(), data.as_slice()])
    );
    let inner: PQInnerData = decode(&tg_tl::bytes_to_primes(&data));
    let inner = inner.c_p_q_inner_data_temp_dc();
    assert_eq!(inner.dc, 10002);
    assert_eq!(inner.expires_in, 86400);
    assert_eq!(inner.nonce, nonce);
    assert_eq!(inner.server_nonce, server_nonce);
    let new_nonce: [u8; 32] = tg_tl::primes_to_bytes(&tg_tl::serialize(&inner.new_nonce))
        .try_into()
        .unwrap();
    let server_nonce_bytes: [u8; 16] = tg_tl::primes_to_bytes(&tg_tl::serialize(&server_nonce))
        .try_into()
        .unwrap();
    let (key, iv) = temporary_aes(&new_nonce, &server_nonce_bytes);
    let prime = BigNum::from_bytes(&GOOD_PRIME);
    let exponent = BigNum::from_bytes(&[0x55; 256]);
    let g_a = BigNum::mod_exp(&BigNum::from_word(3), &exponent, &prime).get_bytes();
    let server_inner =
        mtp_server_dh_inner_data(nonce, server_nonce, 3, GOOD_PRIME.to_vec(), g_a, 1700000000);
    let server_inner = tg_tl::primes_to_bytes(&tg_tl::serialize(&server_inner));
    let encrypted = encrypt_client_dh_inner(&server_inner, &key, &iv, &mut random).unwrap();
    let response = mtp_server_dh_params_ok(nonce, server_nonce, encrypted);
    let mut request = send(
        client
            .receive(
                &tg_tl::serialize(&response),
                std::slice::from_ref(&public),
                Time::default(),
                &mut random,
            )
            .unwrap(),
    );
    let mut previous_aux = None;
    for round in 0..if retry { 2 } else { 1 } {
        let dh: SetClientDHParams = decode(&request);
        assert_eq!(dh.nonce, nonce);
        let mut bytes = dh.encrypted_data;
        aes_ige_decrypt_raw(&mut bytes, &key, &iv).unwrap();
        let words = tg_tl::bytes_to_primes(&bytes[20..]);
        let mut reader = Reader::new(&words);
        let inner = ClientDHInnerData::read(&mut reader).unwrap();
        assert_eq!(bytes[..20], sha1(&bytes[20..20 + reader.position() * 4]));
        let inner = inner.c_client_dh_inner_data();
        assert_eq!(inner.retry_id, previous_aux.unwrap_or(0));
        let auth = BigNum::mod_exp(&BigNum::from_bytes(&inner.g_b), &exponent, &prime);
        let expected = AuthKey::fill_data(&auth.get_bytes());
        let aux = sha1(&expected);
        let tag = if retry && round == 0 { 2 } else { 1 };
        let mut hash_data = new_nonce.to_vec();
        hash_data.push(tag);
        hash_data.extend_from_slice(&aux[..8]);
        let digest = nonce_digest(&hash_data);
        let response = if tag == 2 {
            mtp_dh_gen_retry(nonce, server_nonce, digest)
        } else {
            mtp_dh_gen_ok(nonce, server_nonce, digest)
        };
        let step = client
            .receive(
                &tg_tl::serialize(&response),
                std::slice::from_ref(&public),
                Time {
                    monotonic_ms: 123,
                    unix_seconds: 1700000000,
                },
                &mut random,
            )
            .unwrap();
        match step {
            Step::Send(next) => {
                assert_eq!(tag, 2);
                previous_aux = Some(u64::from_le_bytes(aux[..8].try_into().unwrap()));
                request = next;
            }
            Step::Ready {
                key,
                server_salt,
                server_time,
            } => {
                assert_eq!(tag, 1);
                assert_eq!(key.data(), &expected);
                assert_eq!(key.creation_time(), 123);
                assert_eq!(
                    server_salt,
                    u64::from_le_bytes(new_nonce[..8].try_into().unwrap()) ^ server_nonce.l
                );
                assert_eq!(server_time, 1700000000);
            }
        }
    }
}
