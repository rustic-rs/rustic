use aes256ctr_poly1305aes::{
    aead::{self, Aead, AeadInPlace, NewAead},
    Aes256CtrPoly1305Aes,
};
use rand::{thread_rng, RngCore};
use thiserror::Error;

use super::CryptoKey;

type Nonce = aead::Nonce<Aes256CtrPoly1305Aes>;
type AeadKey = aead::Key<Aes256CtrPoly1305Aes>;

#[derive(Error, Debug)]
pub enum KeyError {
    #[error("crypto error")]
    CryptoError,
}

#[derive(Clone, Default)]
pub struct Key(AeadKey);

impl Key {
    pub fn new() -> Self {
        let mut key = AeadKey::default();
        thread_rng().fill_bytes(&mut key);
        Self(key)
    }

    pub fn from_slice(key: &[u8]) -> Self {
        Self(*AeadKey::from_slice(key))
    }

    pub fn from_keys(encrypt: &[u8], k: &[u8], r: &[u8]) -> Self {
        let mut key = AeadKey::default();
        key[0..32].copy_from_slice(encrypt);
        key[32..48].copy_from_slice(k);
        key[48..64].copy_from_slice(r);

        Self(key)
    }

    pub fn to_keys(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let mut encrypt = vec![0; 32];
        let mut k = vec![0; 16];
        let mut r = vec![0; 16];
        encrypt[0..32].copy_from_slice(&self.0[0..32]);
        k[0..16].copy_from_slice(&self.0[32..48]);
        r[0..16].copy_from_slice(&self.0[48..64]);

        (encrypt, k, r)
    }
}

impl CryptoKey for Key {
    type CryptoError = KeyError;

    fn decrypt_data(&self, data: &[u8]) -> Result<Vec<u8>, Self::CryptoError> {
        if data.len() < 16 {
            return Err(KeyError::CryptoError);
        }

        let nonce = Nonce::from_slice(&data[0..16]);
        Aes256CtrPoly1305Aes::new(&self.0)
            .decrypt(nonce, &data[16..])
            .map_err(|_| KeyError::CryptoError)
    }

    fn encrypt_data(&self, data: &[u8]) -> Result<Vec<u8>, Self::CryptoError> {
        let mut nonce = Nonce::default();
        thread_rng().fill_bytes(&mut nonce);

        let mut res = Vec::with_capacity(data.len() + 32);
        res.extend_from_slice(&nonce);
        res.extend_from_slice(data);
        let tag = Aes256CtrPoly1305Aes::new(&self.0)
            .encrypt_in_place_detached(&nonce, &[], &mut res[16..])
            .map_err(|_| KeyError::CryptoError)?;
        res.extend_from_slice(&tag);
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_hello() {
        let key = Key::default();
        let data: Vec<u8> = b"Hello!".to_vec();
        let enc = key.encrypt_data(&data).unwrap();
        let dec = key.decrypt_data(&enc).unwrap();
        assert_eq!(data, dec);
    }

    #[test]
    fn encrypt_decrypt_empty() {
        let key = Key::default();
        let data = Vec::<u8>::new();
        let enc = key.encrypt_data(&data).unwrap();
        let dec = key.decrypt_data(&enc).unwrap();
        assert_eq!(data, dec);
    }

    #[test]
    fn decrypt_empty() {
        let key = Key::default();
        let data = Vec::<u8>::new();
        let res = key.decrypt_data(&data);
        assert!(res.is_err());
    }
}
