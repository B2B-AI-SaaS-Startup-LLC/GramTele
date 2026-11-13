use aes::cipher::{KeyIvInit, StreamCipher};
use tg_base::openssl_help::sha256_parts;

pub const SALT_SIZE: usize = 64;

pub struct CtrState {
    key: [u8; Self::KEY_SIZE],
    iv: [u8; Self::IV_SIZE],
}

impl CtrState {
    pub const BLOCK_SIZE: usize = 16;
    pub const KEY_SIZE: usize = 32;
    pub const IV_SIZE: usize = Self::BLOCK_SIZE;

    pub fn new(key: &[u8], iv: &[u8]) -> Self {
        Self {
            key: key.try_into().expect("CtrState: wrong key size"),
            iv: iv.try_into().expect("CtrState: wrong IV size"),
        }
    }

    fn incremented_iv(&self, block_index: i64) -> [u8; Self::IV_SIZE] {
        assert!(block_index >= 0);
        u128::from_be_bytes(self.iv)
            .wrapping_add(block_index as u128)
            .to_be_bytes()
    }

    fn process(&self, data: &mut [u8], offset: i64) {
        assert!(data.len().is_multiple_of(Self::BLOCK_SIZE));
        assert_eq!(offset % Self::BLOCK_SIZE as i64, 0);
        let iv = self.incremented_iv(offset / Self::BLOCK_SIZE as i64);
        let mut cipher = ctr::Ctr128BE::<aes::Aes256>::new((&self.key).into(), (&iv).into());
        cipher.apply_keystream(data);
    }

    pub fn encrypt(&self, data: &mut [u8], offset: i64) {
        self.process(data, offset);
    }

    pub fn decrypt(&self, data: &mut [u8], offset: i64) {
        self.process(data, offset);
    }
}

#[derive(Clone, Default)]
pub struct EncryptionKey {
    data: Vec<u8>,
}

impl EncryptionKey {
    pub const SIZE: usize = 256;
    pub const SIZE_V2: usize = 64;

    pub fn new(data: Vec<u8>) -> Self {
        assert!(data.len() == Self::SIZE || data.len() == Self::SIZE_V2);
        Self { data }
    }

    pub fn empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn prepare_ctr_state(&self, salt: &[u8]) -> CtrState {
        assert_eq!(salt.len(), SALT_SIZE);
        assert!(!self.empty());
        let (first, second) = self.data.split_at(self.data.len() / 2);
        let key = sha256_parts(&[first, &salt[..SALT_SIZE / 2]]);
        let iv = sha256_parts(&[second, &salt[SALT_SIZE / 2..]]);
        CtrState::new(&key, &iv[..CtrState::IV_SIZE])
    }
}
