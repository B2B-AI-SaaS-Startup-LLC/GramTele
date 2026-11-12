//! RSA public operations use the same big-endian BigNum as DH, no implicit padding.
use crate::core_types::Random;
use tg_base::openssl_help::{sha1, BigNum};
#[derive(Clone, Debug)]
pub struct RsaPublicKey {
    n: Vec<u8>,
    e: Vec<u8>,
    fingerprint: u64,
}
impl RsaPublicKey {
    pub fn from_components(n: &[u8], e: &[u8]) -> Option<Self> {
        let n = BigNum::from_bytes(n).get_bytes();
        let e = BigNum::from_bytes(e).get_bytes();
        if n.is_empty() || e.is_empty() {
            return None;
        }
        let mut buffer = Vec::new();
        tg_tl::write_string(&mut buffer, &n);
        tg_tl::write_string(&mut buffer, &e);
        let hash = sha1(&tg_tl::primes_to_bytes(&buffer));
        let fingerprint = u64::from_le_bytes(hash[12..].try_into().unwrap());
        Some(Self { n, e, fingerprint })
    }
    pub fn from_pem(pem: &str) -> Option<Self> {
        let encoded: String = pem
            .lines()
            .filter(|line| !line.starts_with("---"))
            .collect();
        let der = base64_decode(&encoded)?;
        let mut outer = der.as_slice();
        let mut sequence = der_field(&mut outer, 0x30)?;
        if pem.contains("BEGIN PUBLIC KEY") {
            der_field(&mut sequence, 0x30)?;
            let bits = der_field(&mut sequence, 3)?;
            if bits.first() != Some(&0) {
                return None;
            }
            let mut nested = &bits[1..];
            sequence = der_field(&mut nested, 0x30)?;
        } else if !pem.contains("BEGIN RSA PUBLIC KEY") {
            return None;
        }
        let n = der_field(&mut sequence, 2)?;
        let e = der_field(&mut sequence, 2)?;
        if !sequence.is_empty() {
            return None;
        }
        Self::from_components(n, e)
    }
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    pub fn get_n(&self) -> &[u8] {
        &self.n
    }
    pub fn get_e(&self) -> &[u8] {
        &self.e
    }
    pub fn encrypt(&self, input: &[u8]) -> Option<Vec<u8>> {
        if input.len() != 256 || self.n.len() != 256 {
            return None;
        }
        self.public_operation(input)
    }
    /// Original RSA_public_decrypt with RSA_NO_PADDING is the same public operation.
    pub fn decrypt(&self, input: &[u8]) -> Option<Vec<u8>> {
        self.encrypt(input)
    }
    fn public_operation(&self, input: &[u8]) -> Option<Vec<u8>> {
        let n = BigNum::from_bytes(&self.n);
        let input = BigNum::from_bytes(input);
        if BigNum::compare(&input, &n) >= 0 {
            return None;
        }
        let out = BigNum::mod_exp(&input, &BigNum::from_bytes(&self.e), &n);
        if out.failed() {
            return None;
        }
        let bytes = out.get_bytes();
        let mut result = vec![0; self.n.len()];
        let offset = result.len().checked_sub(bytes.len())?;
        result[offset..].copy_from_slice(&bytes);
        Some(result)
    }
    /// OpenSSL's default OAEP parameters: SHA-1, MGF1-SHA1, empty label.
    pub fn encrypt_oaep_padding(&self, input: &[u8], random: &mut impl Random) -> Option<Vec<u8>> {
        let size = self.n.len();
        if size < 42 || input.len() > size - 42 {
            return None;
        }
        let mut seed = [0; 20];
        random.fill(&mut seed);
        let mut db = sha1(&[]);
        db.resize(size - 21 - input.len() - 1, 0);
        db.push(1);
        db.extend_from_slice(input);
        let mask = mgf1(&seed, db.len());
        for (a, b) in db.iter_mut().zip(mask) {
            *a ^= b;
        }
        let mask = mgf1(&db, 20);
        for (a, b) in seed.iter_mut().zip(mask) {
            *a ^= b;
        }
        let mut padded = vec![0];
        padded.extend(seed);
        padded.extend(db);
        self.public_operation(&padded)
    }
}
fn mgf1(seed: &[u8], size: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(size + 20);
    for count in 0..size.div_ceil(20) {
        let mut bytes = seed.to_vec();
        bytes.extend((count as u32).to_be_bytes());
        result.extend(sha1(&bytes));
    }
    result.truncate(size);
    result
}
fn der_field<'a>(data: &mut &'a [u8], tag: u8) -> Option<&'a [u8]> {
    if data.first() != Some(&tag) {
        return None;
    }
    let first = *data.get(1)?;
    let mut offset = 2;
    let size = if first & 128 == 0 {
        first as usize
    } else {
        let count = (first & 127) as usize;
        if count == 0 || count > 4 {
            return None;
        }
        let mut size = 0usize;
        for &b in data.get(2..2 + count)? {
            size = size.checked_mul(256)?.checked_add(b as usize)?;
        }
        offset += count;
        size
    };
    let end = offset.checked_add(size)?;
    let value = data.get(offset..end)?;
    *data = data.get(end..)?;
    Some(value)
}
/// Qt accepts optional padding in both standard and URL alphabets.
pub(crate) fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut result = Vec::new();
    let mut bits = 0u32;
    let mut count = 0;
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b' ' | b'\r' | b'\n' | b'\t' => continue,
            _ => return None,
        };
        bits = (bits << 6) | value as u32;
        count += 6;
        if count >= 8 {
            count -= 8;
            result.push((bits >> count) as u8);
        }
    }
    Some(result)
}
pub(crate) fn base64_url_encode(bytes: &[u8]) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let mut bits = 0u32;
    let mut count = 0;
    for &b in bytes {
        bits = (bits << 8) | b as u32;
        count += 8;
        while count >= 6 {
            count -= 6;
            out.push(ABC[((bits >> count) & 63) as usize] as char);
        }
    }
    if count > 0 {
        out.push(ABC[((bits << (6 - count)) & 63) as usize] as char);
    }
    out
}
