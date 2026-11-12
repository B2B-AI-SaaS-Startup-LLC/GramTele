//! req_pq_multi → RSA_PAD → DH, driven by input bytes and explicit time/entropy.
use super::mtproto_rsa_public_key::RsaPublicKey;
use crate::{
    core_types::{Random, Time},
    mtproto_auth_key::{aes_ige_decrypt_raw, aes_ige_encrypt_raw, AuthKey, KeyType},
    mtproto_dh_utils::{create_auth_key, create_mod_exp, is_prime_and_good},
};
use std::sync::Arc;
use tg_base::openssl_help::{sha1, sha1_parts, sha256, sha256_parts, BigNum};
use tg_scheme::*;
use tg_tl::{Buffer, Deserialize, Int128, Int256, Reader};
use zeroize::Zeroize;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DcKeyError {
    UnknownPublicKey,
    Invalid(&'static str),
}
impl std::fmt::Display for DcKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPublicKey => f.write_str("unknown RSA fingerprint"),
            Self::Invalid(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for DcKeyError {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    None,
    WaitingPQ,
    WaitingDH,
    WaitingDone,
    Ready,
    Failed,
}
pub enum Step {
    Send(Buffer),
    Ready {
        key: Arc<AuthKey>,
        server_salt: u64,
        server_time: i32,
    },
}
pub struct KeyExchange {
    pub stage: Stage,
    dc_id: i32,
    protocol_dc_id: i16,
    expires_in: i32,
    nonce: Int128,
    server_nonce: Int128,
    new_nonce: [u8; 32],
    aes_key: [u8; 32],
    aes_iv: [u8; 32],
    prime: Vec<u8>,
    g: i32,
    g_a: Vec<u8>,
    retry_id: u64,
    retries: u32,
    auth_key: [u8; 256],
    aux_hash: [u8; 8],
    server_time: i32,
    sync_time: Option<i32>,
}
impl Drop for KeyExchange {
    fn drop(&mut self) {
        self.new_nonce.zeroize();
        self.aes_key.zeroize();
        self.aes_iv.zeroize();
        self.auth_key.zeroize();
        self.aux_hash.zeroize();
    }
}
fn invalid(s: &'static str) -> DcKeyError {
    DcKeyError::Invalid(s)
}
fn read<T: Deserialize>(words: &[i32]) -> Result<T, DcKeyError> {
    T::read(&mut Reader::new(words)).map_err(|_| invalid("malformed TL response"))
}
fn nonce_bytes(n: Int128) -> [u8; 16] {
    tg_tl::primes_to_bytes(&tg_tl::serialize(&n))
        .try_into()
        .unwrap()
}
fn int256(bytes: &[u8; 32]) -> Int256 {
    tg_tl::deserialize_exact(&tg_tl::bytes_to_primes(bytes)).unwrap()
}
pub fn nonce_digest(data: &[u8]) -> Int128 {
    tg_tl::deserialize_exact(&tg_tl::bytes_to_primes(&sha1(data)[4..20])).unwrap()
}
impl KeyExchange {
    pub fn new(dc_id: i32, protocol_dc_id: i16, expires_in: i32) -> Self {
        Self {
            stage: Stage::None,
            dc_id,
            protocol_dc_id,
            expires_in,
            nonce: Int128::default(),
            server_nonce: Int128::default(),
            new_nonce: [0; 32],
            aes_key: [0; 32],
            aes_iv: [0; 32],
            prime: Vec::new(),
            g: 0,
            g_a: Vec::new(),
            retry_id: 0,
            retries: 0,
            auth_key: [0; 256],
            aux_hash: [0; 8],
            server_time: 0,
            sync_time: None,
        }
    }
    pub fn take_sync_time(&mut self) -> Option<i32> {
        self.sync_time.take()
    }
    pub fn start(&mut self, random: &mut impl Random) -> Result<Buffer, DcKeyError> {
        if self.stage != Stage::None {
            return Err(invalid("unexpected start stage"));
        }
        self.nonce = Int128 {
            l: random.u64(),
            h: random.u64(),
        };
        self.stage = Stage::WaitingPQ;
        Ok(tg_tl::serialize(&ReqPqMulti::new(self.nonce)))
    }
    pub fn receive(
        &mut self,
        words: &[i32],
        keys: &[RsaPublicKey],
        time: Time,
        random: &mut impl Random,
    ) -> Result<Step, DcKeyError> {
        let result = (|| match self.stage {
            Stage::WaitingPQ => self.pq_answered(read(words)?, keys, random),
            Stage::WaitingDH => self.dh_params_answered(read(words)?, random),
            Stage::WaitingDone => self.dh_client_params_answered(read(words)?, time, random),
            _ => Err(invalid("unexpected exchange stage")),
        })();
        if result.is_err() {
            self.stage = Stage::Failed;
        }
        result
    }
    fn pq_answered(
        &mut self,
        response: ResPQ,
        keys: &[RsaPublicKey],
        random: &mut impl Random,
    ) -> Result<Step, DcKeyError> {
        let data = response.c_res_pq();
        if data.nonce != self.nonce {
            return Err(invalid("resPQ nonce mismatch"));
        }
        let key = data
            .server_public_key_fingerprints
            .iter()
            .find_map(|fp| keys.iter().find(|key| key.fingerprint() == *fp))
            .ok_or(DcKeyError::UnknownPublicKey)?;
        self.server_nonce = data.server_nonce;
        random.fill(&mut self.new_nonce);
        let (p, q) =
            factorize_pq(&data.pq, random).ok_or_else(|| invalid("could not factor pq"))?;
        let inner = if self.expires_in == 0 {
            mtp_p_q_inner_data_dc(
                data.pq.clone(),
                p.clone(),
                q.clone(),
                self.nonce,
                self.server_nonce,
                int256(&self.new_nonce),
                self.protocol_dc_id as i32,
            )
        } else {
            mtp_p_q_inner_data_temp_dc(
                data.pq.clone(),
                p.clone(),
                q.clone(),
                self.nonce,
                self.server_nonce,
                int256(&self.new_nonce),
                self.protocol_dc_id as i32,
                self.expires_in,
            )
        };
        let encrypted = encrypt_pq_inner_rsa(
            &tg_tl::primes_to_bytes(&tg_tl::serialize(&inner)),
            key,
            random,
        )?;
        self.stage = Stage::WaitingDH;
        Ok(Step::Send(tg_tl::serialize(&ReqDHParams::new(
            self.nonce,
            self.server_nonce,
            p,
            q,
            key.fingerprint(),
            encrypted,
        ))))
    }
    fn dh_params_answered(
        &mut self,
        response: ServerDHParams,
        random: &mut impl Random,
    ) -> Result<Step, DcKeyError> {
        match response {
            ServerDHParams::ServerDHParamsFail(d) => {
                if d.nonce != self.nonce || d.server_nonce != self.server_nonce {
                    return Err(invalid("server_DH_params_fail nonce mismatch"));
                }
                if d.new_nonce_hash != nonce_digest(&self.new_nonce) {
                    return Err(invalid("server_DH_params_fail hash mismatch"));
                }
                Err(invalid("server_DH_params_fail"))
            }
            ServerDHParams::ServerDHParamsOk(d) => {
                if d.nonce != self.nonce || d.server_nonce != self.server_nonce {
                    return Err(invalid("server_DH_params_ok nonce mismatch"));
                }
                if d.encrypted_answer.len() < 24 || !d.encrypted_answer.len().is_multiple_of(16) {
                    return Err(invalid("bad encrypted DH length"));
                }
                (self.aes_key, self.aes_iv) =
                    temporary_aes(&self.new_nonce, &nonce_bytes(self.server_nonce));
                let mut decrypted = d.encrypted_answer.clone();
                aes_ige_decrypt_raw(&mut decrypted, &self.aes_key, &self.aes_iv)
                    .map_err(invalid)?;
                let words = tg_tl::bytes_to_primes(&decrypted[20..]);
                let mut reader = Reader::new(&words);
                let inner = ServerDHInnerData::read(&mut reader)
                    .map_err(|_| invalid("could not decrypt server_DH_inner_data"))?;
                let data = inner.c_server_dh_inner_data();
                if data.nonce != self.nonce || data.server_nonce != self.server_nonce {
                    return Err(invalid("server_DH_inner_data nonce mismatch"));
                }
                if decrypted[..20] != sha1(&decrypted[20..20 + reader.position() * 4]) {
                    return Err(invalid("server_DH_inner_data SHA1 mismatch"));
                }
                self.sync_time = Some(data.server_time);
                if !is_prime_and_good(&data.dh_prime, data.g) {
                    return Err(invalid("bad DH prime or generator"));
                }
                self.server_time = data.server_time;
                self.prime = data.dh_prime.clone();
                self.g = data.g;
                self.g_a = data.g_a.clone();
                self.retry_id = 0;
                self.retries = 0;
                self.dh_client_params_send(random)
            }
        }
    }
    fn dh_client_params_send(&mut self, random: &mut impl Random) -> Result<Step, DcKeyError> {
        self.retries += 1;
        if self.retries > 5 {
            return Err(invalid("DH retry limit exceeded"));
        }
        let mut seed = [0; 256];
        random.fill(&mut seed);
        let first = create_mod_exp(self.g, &self.prime, &seed, random);
        seed.zeroize();
        let mut power = first.random_power;
        let computed = create_auth_key(&self.g_a, &power, &self.prime);
        power.zeroize();
        let mut computed = computed.ok_or_else(|| invalid("bad server g_a"))?;
        self.auth_key = AuthKey::fill_data(&computed);
        computed.zeroize();
        let hash = sha1(&self.auth_key);
        self.aux_hash.copy_from_slice(&hash[..8]);
        let data =
            mtp_client_dh_inner_data(self.nonce, self.server_nonce, self.retry_id, first.modexp);
        let encrypted = encrypt_client_dh_inner(
            &tg_tl::primes_to_bytes(&tg_tl::serialize(&data)),
            &self.aes_key,
            &self.aes_iv,
            random,
        )?;
        self.stage = Stage::WaitingDone;
        Ok(Step::Send(tg_tl::serialize(&SetClientDHParams::new(
            self.nonce,
            self.server_nonce,
            encrypted,
        ))))
    }
    fn dh_client_params_answered(
        &mut self,
        response: SetClientDHParamsAnswer,
        time: Time,
        random: &mut impl Random,
    ) -> Result<Step, DcKeyError> {
        let (nonce, server_nonce, digest, tag) = match response {
            SetClientDHParamsAnswer::DhGenOk(d) => (d.nonce, d.server_nonce, d.new_nonce_hash1, 1),
            SetClientDHParamsAnswer::DhGenRetry(d) => {
                (d.nonce, d.server_nonce, d.new_nonce_hash2, 2)
            }
            SetClientDHParamsAnswer::DhGenFail(d) => {
                (d.nonce, d.server_nonce, d.new_nonce_hash3, 3)
            }
        };
        if nonce != self.nonce || server_nonce != self.server_nonce {
            return Err(invalid("dh_gen nonce mismatch"));
        }
        let mut data = self.new_nonce.to_vec();
        data.push(tag);
        data.extend(self.aux_hash);
        if digest != nonce_digest(&data) {
            return Err(invalid("dh_gen nonce hash mismatch"));
        }
        match tag {
            1 => {
                self.stage = Stage::Ready;
                let key = AuthKey::new(
                    if self.expires_in > 0 {
                        KeyType::Temporary
                    } else {
                        KeyType::Generated
                    },
                    self.dc_id,
                    self.auth_key,
                    time.monotonic_ms,
                );
                let salt = u64::from_le_bytes(self.new_nonce[..8].try_into().unwrap())
                    ^ self.server_nonce.l;
                Ok(Step::Ready {
                    key: Arc::new(key),
                    server_salt: salt,
                    server_time: self.server_time,
                })
            }
            2 => {
                self.retry_id = u64::from_le_bytes(self.aux_hash);
                self.dh_client_params_send(random)
            }
            _ => Err(invalid("dh_gen_fail")),
        }
    }
}
pub fn temporary_aes(new_nonce: &[u8; 32], server_nonce: &[u8; 16]) -> ([u8; 32], [u8; 32]) {
    let ns = sha1_parts(&[new_nonce.as_slice(), server_nonce.as_slice()]);
    let sn = sha1_parts(&[server_nonce.as_slice(), new_nonce.as_slice()]);
    let nn = sha1_parts(&[new_nonce.as_slice(), new_nonce.as_slice()]);
    (
        [&ns[..], &sn[..12]].concat().try_into().unwrap(),
        [&sn[12..], &nn[..], &new_nonce[..4]]
            .concat()
            .try_into()
            .unwrap(),
    )
}
pub fn encrypt_client_dh_inner(
    data: &[u8],
    key: &[u8; 32],
    iv: &[u8; 32],
    random: &mut impl Random,
) -> Result<Vec<u8>, DcKeyError> {
    let mut bytes = sha1(data);
    bytes.extend_from_slice(data);
    let plain = bytes.len();
    bytes.resize(plain.next_multiple_of(16), 0);
    random.fill(&mut bytes[plain..]);
    aes_ige_encrypt_raw(&mut bytes, key, iv).map_err(invalid)?;
    Ok(bytes)
}
pub fn encrypt_pq_inner_rsa(
    data: &[u8],
    key: &RsaPublicKey,
    random: &mut impl Random,
) -> Result<Vec<u8>, DcKeyError> {
    if data.len() > 144 {
        return Err(invalid("PQ inner exceeds 144 bytes"));
    }
    let mut padded = [0; 192];
    padded[..data.len()].copy_from_slice(data);
    random.fill(&mut padded[data.len()..]);
    loop {
        let mut temp_key = [0; 32];
        random.fill(&mut temp_key);
        let hash = sha256_parts(&[temp_key.as_slice(), padded.as_slice()]);
        let mut encrypted = padded.to_vec();
        encrypted.reverse();
        encrypted.extend(hash);
        aes_ige_encrypt_raw(&mut encrypted, &temp_key, &[0; 32]).map_err(invalid)?;
        let hash = sha256(&encrypted);
        let mut input = Vec::with_capacity(256);
        input.extend(temp_key.iter().zip(hash).map(|(a, b)| a ^ b));
        temp_key.zeroize();
        input.extend(encrypted);
        if BigNum::compare(
            &BigNum::from_bytes(&input),
            &BigNum::from_bytes(key.get_n()),
        ) < 0
        {
            return key
                .encrypt(&input)
                .ok_or_else(|| invalid("RSA public encryption failed"));
        }
    }
}
fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
pub fn factorize_pq(bytes: &[u8], random: &mut impl Random) -> Option<(Vec<u8>, Vec<u8>)> {
    if bytes.len() > 8 || bytes.len() == 8 && bytes[0] & 128 != 0 {
        return factorize_big_pq(bytes, random);
    }
    let mut array = [0; 8];
    array[8 - bytes.len()..].copy_from_slice(bytes);
    let pq = u64::from_be_bytes(array);
    if pq < 2 {
        return Some((
            1u32.to_be_bytes().to_vec(),
            (pq as u32).to_be_bytes().to_vec(),
        ));
    }
    let mut factor = 0;
    let mut iter = 0;
    let mut attempt = 0;
    while attempt < 3 || iter < 1000 {
        let q = (17 + random_index(16, random) as u64) % (pq - 1);
        let mut x = random.u64() % (pq - 1) + 1;
        let mut y = x;
        let limit = 1 << (attempt.min(5) + 18);
        for j in 1..limit {
            iter += 1;
            x = ((x as u128 * x as u128 + q as u128) % (pq as u128)) as u64;
            factor = gcd(x.abs_diff(y), pq);
            if factor != 1 {
                break;
            }
            if j & (j - 1) == 0 {
                y = x;
            }
        }
        if factor > 1 && factor < pq {
            break;
        }
        attempt += 1;
    }
    if factor == 0 || pq % factor != 0 {
        return None;
    }
    factor = factor.min(pq / factor);
    Some((
        (factor as u32).to_be_bytes().to_vec(),
        ((pq / factor) as u32).to_be_bytes().to_vec(),
    ))
}
fn random_index(count: u32, random: &mut impl Random) -> u32 {
    let max = u32::MAX / count * count;
    loop {
        let x = random.u32();
        if x < max {
            return x % count;
        }
    }
}
fn factorize_big_pq(bytes: &[u8], random: &mut impl Random) -> Option<(Vec<u8>, Vec<u8>)> {
    let pq = BigNum::from_bytes(bytes);
    let one = BigNum::from_word(1);
    let mut iterations = 0;
    for attempt in 0u32.. {
        let t = BigNum::from_word(17 + random_index(16, random));
        let mut a = BigNum::from_word(random.u32());
        let mut b = a.clone();
        let limit = 1u64.checked_shl(attempt + 23)?;
        for j in 1..limit {
            iterations += 1;
            a = BigNum::mod_mul(&a, &a, &pq);
            a = BigNum::add(&a, &t);
            if BigNum::compare(&a, &pq) >= 0 {
                a = BigNum::sub(&a, &pq);
            }
            let difference = if BigNum::compare(&a, &b) > 0 {
                BigNum::sub(&a, &b)
            } else {
                BigNum::sub(&b, &a)
            };
            let p = BigNum::gcd(&difference, &pq);
            if BigNum::compare(&p, &one) != 0 {
                let mut q = BigNum::new();
                BigNum::div(Some(&mut q), None, &pq, &p);
                return Some(if BigNum::compare(&p, &q) > 0 {
                    (q.get_bytes(), p.get_bytes())
                } else {
                    (p.get_bytes(), q.get_bytes())
                });
            }
            if j & (j - 1) == 0 {
                b = a.clone();
            }
        }
        if attempt >= 2 && iterations >= 1000 {
            break;
        }
    }
    None
}
#[derive(Clone, Copy)]
pub struct DcKeyRequest {
    pub temporary_expires_in: i32,
    pub persistent_needed: bool,
}
pub struct DcKeyResult {
    pub persistent_key: Option<Arc<AuthKey>>,
    pub temporary_key: Arc<AuthKey>,
    pub temporary_server_salt: u64,
    pub persistent_server_salt: u64,
}

/// Coordinates the persistent exchange followed by temporary exchange as in C++.
pub struct DcKeyCreator {
    active: KeyExchange,
    request: DcKeyRequest,
    persistent: Option<Arc<AuthKey>>,
    persistent_salt: u64,
}
pub enum CreationStep {
    Send(Buffer),
    Ready(DcKeyResult),
}
impl DcKeyCreator {
    pub fn take_sync_time(&mut self) -> Option<i32> {
        self.active.take_sync_time()
    }
    pub fn new(dc_id: i32, protocol_dc_id: i16, request: DcKeyRequest) -> Self {
        assert!(request.temporary_expires_in > 0);
        Self {
            active: KeyExchange::new(
                dc_id,
                protocol_dc_id,
                if request.persistent_needed {
                    0
                } else {
                    request.temporary_expires_in
                },
            ),
            request,
            persistent: None,
            persistent_salt: 0,
        }
    }
    pub fn start(&mut self, random: &mut impl Random) -> Result<Buffer, DcKeyError> {
        self.active.start(random)
    }
    pub fn receive(
        &mut self,
        words: &[i32],
        keys: &[RsaPublicKey],
        time: Time,
        random: &mut impl Random,
    ) -> Result<CreationStep, DcKeyError> {
        match self.active.receive(words, keys, time, random)? {
            Step::Send(data) => Ok(CreationStep::Send(data)),
            Step::Ready {
                key, server_salt, ..
            } => {
                if self.request.persistent_needed && self.persistent.is_none() {
                    self.persistent = Some(key);
                    self.persistent_salt = server_salt;
                    self.active = KeyExchange::new(
                        self.active.dc_id,
                        self.active.protocol_dc_id,
                        self.request.temporary_expires_in,
                    );
                    Ok(CreationStep::Send(self.active.start(random)?))
                } else {
                    Ok(CreationStep::Ready(DcKeyResult {
                        persistent_key: self.persistent.clone(),
                        temporary_key: key,
                        temporary_server_salt: server_salt,
                        persistent_server_salt: self.persistent_salt,
                    }))
                }
            }
        }
    }
}

/// Live adapters and memory servers drive the same state machine. Deadlines are
/// owned by the caller; no global clock or socket is consulted by KeyExchange.
pub async fn create_key<
    C: crate::connection_abstract::Exchange,
    T: crate::connection_abstract::Clock,
>(
    connection: &mut C,
    clock: &T,
    dc_id: i32,
    protocol_dc_id: i16,
    expires_in: i32,
    keys: &[RsaPublicKey],
    random: &mut impl Random,
) -> Result<(Arc<AuthKey>, u64), Box<dyn std::error::Error>> {
    let mut exchange = KeyExchange::new(dc_id, protocol_dc_id, expires_in);
    let mut body = exchange.start(random)?;
    loop {
        let id = clock.next_message_id();
        let mut packet = vec![0, 0, 0, 0, id as i32, (id >> 32) as i32, 0];
        packet.extend(body);
        let mut b = [0];
        random.fill(&mut b);
        let padding = (b[0] & 63) as usize;
        packet[6] = ((packet.len() - 7 + padding) * 4) as i32;
        for _ in 0..padding {
            packet.push(random.u32() as i32);
        }
        let response = connection.exchange(packet).await?;
        let words = crate::connection_abstract::parse_not_secure_response(&response)?;
        let step = exchange.receive(words, keys, clock.now(), random);
        if let Some(server_time) = exchange.take_sync_time() {
            clock.update_unix_time(server_time);
        }
        match step? {
            Step::Send(next) => body = next,
            Step::Ready {
                key, server_salt, ..
            } => return Ok((key, server_salt)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Entropy(u64);
    impl Random for Entropy {
        fn fill(&mut self, b: &mut [u8]) {
            for x in b {
                self.0 ^= self.0 << 13;
                self.0 ^= self.0 >> 7;
                self.0 ^= self.0 << 17;
                *x = self.0 as u8;
            }
        }
    }
    fn base(stage: Stage) -> KeyExchange {
        let mut x = KeyExchange::new(2, 10002, 86400);
        x.stage = stage;
        x.nonce = Int128 { l: 1, h: 2 };
        x.server_nonce = Int128 { l: 3, h: 4 };
        x.new_nonce = [5; 32];
        x.aux_hash = [6; 8];
        x
    }
    fn check(mut x: KeyExchange, words: Buffer, expected: &str) {
        let error = x
            .receive(&words, &[], Time::default(), &mut Entropy(42))
            .err()
            .expect("expected rejection");
        assert_eq!(error.to_string(), expected);
        assert_eq!(x.stage, Stage::Failed);
    }
    fn ok_response(
        x: &KeyExchange,
        nonce: Int128,
        server_nonce: Int128,
        prime: Vec<u8>,
        g: i32,
        hash_good: bool,
    ) -> Buffer {
        let inner = mtp_server_dh_inner_data(nonce, server_nonce, g, prime, vec![1], 1700000000);
        let bytes = tg_tl::primes_to_bytes(&tg_tl::serialize(&inner));
        let (key, iv) = temporary_aes(&x.new_nonce, &nonce_bytes(x.server_nonce));
        let mut encoded = sha1(&bytes);
        if !hash_good {
            encoded[0] ^= 1;
        }
        encoded.extend(bytes);
        encoded.resize(encoded.len().next_multiple_of(16), 0);
        aes_ige_encrypt_raw(&mut encoded, &key, &iv).unwrap();
        tg_tl::serialize(&mtp_server_dh_params_ok(x.nonce, x.server_nonce, encoded))
    }
    #[test]
    fn unexpected_stage() {
        check(base(Stage::None), vec![0], "unexpected exchange stage");
    }
    #[test]
    fn malformed_tl_is_terminal() {
        for stage in [Stage::WaitingPQ, Stage::WaitingDH, Stage::WaitingDone] {
            check(base(stage), vec![0], "malformed TL response");
        }
    }
    #[test]
    fn start_twice() {
        let mut x = KeyExchange::new(2, 10002, 0);
        x.start(&mut Entropy(42)).unwrap();
        assert_eq!(
            x.start(&mut Entropy(42)).unwrap_err().to_string(),
            "unexpected start stage"
        );
    }
    #[test]
    fn pq_wrong_nonce() {
        check(
            base(Stage::WaitingPQ),
            tg_tl::serialize(&mtp_res_pq(
                Int128::default(),
                Int128::default(),
                vec![1],
                vec![],
            )),
            "resPQ nonce mismatch",
        );
    }
    #[test]
    fn unknown_public_key() {
        let x = base(Stage::WaitingPQ);
        let response = tg_tl::serialize(&mtp_res_pq(x.nonce, x.server_nonce, vec![1], vec![1]));
        check(x, response, "unknown RSA fingerprint");
    }
    #[test]
    fn dh_ok_outer_nonce() {
        for which in 0..2 {
            let x = base(Stage::WaitingDH);
            let response = tg_tl::serialize(&mtp_server_dh_params_ok(
                if which == 0 {
                    Int128::default()
                } else {
                    x.nonce
                },
                if which == 1 {
                    Int128::default()
                } else {
                    x.server_nonce
                },
                vec![0; 32],
            ));
            check(x, response, "server_DH_params_ok nonce mismatch");
        }
    }
    #[test]
    fn dh_bad_encrypted_length() {
        for size in [0, 16, 31, 33] {
            let x = base(Stage::WaitingDH);
            let response = tg_tl::serialize(&mtp_server_dh_params_ok(
                x.nonce,
                x.server_nonce,
                vec![0; size],
            ));
            check(x, response, "bad encrypted DH length");
        }
    }
    #[test]
    fn dh_cannot_decrypt() {
        let x = base(Stage::WaitingDH);
        let response = tg_tl::serialize(&mtp_server_dh_params_ok(
            x.nonce,
            x.server_nonce,
            vec![0; 32],
        ));
        check(x, response, "could not decrypt server_DH_inner_data");
    }
    #[test]
    fn dh_inner_nonce() {
        for which in 0..2 {
            let x = base(Stage::WaitingDH);
            let response = ok_response(
                &x,
                if which == 0 {
                    Int128::default()
                } else {
                    x.nonce
                },
                if which == 1 {
                    Int128::default()
                } else {
                    x.server_nonce
                },
                vec![7],
                3,
                true,
            );
            check(x, response, "server_DH_inner_data nonce mismatch");
        }
    }
    #[test]
    fn dh_inner_hash() {
        let x = base(Stage::WaitingDH);
        let response = ok_response(&x, x.nonce, x.server_nonce, vec![7], 3, false);
        check(x, response, "server_DH_inner_data SHA1 mismatch");
    }
    #[test]
    fn dh_prime_rejection() {
        let x = base(Stage::WaitingDH);
        let response = ok_response(&x, x.nonce, x.server_nonce, vec![7], 3, true);
        check(x, response, "bad DH prime or generator");
    }
    #[test]
    fn dh_bad_ga() {
        let x = base(Stage::WaitingDH);
        let response = ok_response(
            &x,
            x.nonce,
            x.server_nonce,
            crate::mtproto_dh_utils::GOOD_PRIME.to_vec(),
            3,
            true,
        );
        check(x, response, "bad server g_a");
    }
    #[test]
    fn dh_fail_nonce() {
        let x = base(Stage::WaitingDH);
        let response = tg_tl::serialize(&mtp_server_dh_params_fail(
            x.nonce,
            Int128::default(),
            Int128::default(),
        ));
        check(x, response, "server_DH_params_fail nonce mismatch");
    }
    #[test]
    fn dh_fail_hash() {
        let x = base(Stage::WaitingDH);
        let response = tg_tl::serialize(&mtp_server_dh_params_fail(
            x.nonce,
            x.server_nonce,
            Int128::default(),
        ));
        check(x, response, "server_DH_params_fail hash mismatch");
    }
    #[test]
    fn dh_fail_valid_hash() {
        let x = base(Stage::WaitingDH);
        let response = tg_tl::serialize(&mtp_server_dh_params_fail(
            x.nonce,
            x.server_nonce,
            nonce_digest(&x.new_nonce),
        ));
        check(x, response, "server_DH_params_fail");
    }
    fn gen(tag: u8, nonce: Int128, server_nonce: Int128, hash: Int128) -> SetClientDHParamsAnswer {
        match tag {
            1 => mtp_dh_gen_ok(nonce, server_nonce, hash),
            2 => mtp_dh_gen_retry(nonce, server_nonce, hash),
            _ => mtp_dh_gen_fail(nonce, server_nonce, hash),
        }
    }
    #[test]
    fn gen_wrong_nonces() {
        for tag in 1..=3 {
            for which in 0..2 {
                let x = base(Stage::WaitingDone);
                let response = tg_tl::serialize(&gen(
                    tag,
                    if which == 0 {
                        Int128::default()
                    } else {
                        x.nonce
                    },
                    if which == 1 {
                        Int128::default()
                    } else {
                        x.server_nonce
                    },
                    Int128::default(),
                ));
                check(x, response, "dh_gen nonce mismatch");
            }
        }
    }
    #[test]
    fn gen_wrong_hashes() {
        for tag in 1..=3 {
            let x = base(Stage::WaitingDone);
            let response = tg_tl::serialize(&gen(tag, x.nonce, x.server_nonce, Int128::default()));
            check(x, response, "dh_gen nonce hash mismatch");
        }
    }
    #[test]
    fn gen_fail_valid_hash() {
        let x = base(Stage::WaitingDone);
        let mut bytes = x.new_nonce.to_vec();
        bytes.push(3);
        bytes.extend(x.aux_hash);
        let response = tg_tl::serialize(&gen(3, x.nonce, x.server_nonce, nonce_digest(&bytes)));
        check(x, response, "dh_gen_fail");
    }
    #[test]
    fn gen_retry_limit() {
        let mut x = base(Stage::WaitingDone);
        x.retries = 5;
        let mut bytes = x.new_nonce.to_vec();
        bytes.push(2);
        bytes.extend(x.aux_hash);
        let response = tg_tl::serialize(&gen(2, x.nonce, x.server_nonce, nonce_digest(&bytes)));
        check(x, response, "DH retry limit exceeded");
    }
    #[test]
    fn rsa_pad_too_large() {
        let key = RsaPublicKey::from_components(&[255; 256], &[1, 0, 1]).unwrap();
        assert_eq!(
            encrypt_pq_inner_rsa(&[0; 145], &key, &mut Entropy(42))
                .unwrap_err()
                .to_string(),
            "PQ inner exceeds 144 bytes"
        );
    }
    #[test]
    fn pq_factor_bytes_match_original_padding() {
        let (p, q) = factorize_pq(&323u16.to_be_bytes(), &mut Entropy(42)).unwrap();
        assert_eq!(p, 17u32.to_be_bytes());
        assert_eq!(q, 19u32.to_be_bytes());
        let (p, q) = factorize_pq(&[], &mut Entropy(42)).unwrap();
        assert_eq!(p, 1u32.to_be_bytes());
        assert_eq!(q, 0u32.to_be_bytes());
    }
}
