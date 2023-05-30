use aes256ctr_poly1305aes::{
    aead::{self, Aead, AeadInPlace, NewAead},
    Aes256CtrPoly1305Aes,
};
use rand::{thread_rng, RngCore};

use crate::{crypto::CryptoKey, error::CryptoErrorKind, RusticResult};

pub(crate) type Nonce = aead::Nonce<Aes256CtrPoly1305Aes>;
pub(crate) type AeadKey = aead::Key<Aes256CtrPoly1305Aes>;

#[derive(Clone, Default, Debug, Copy)]
pub struct Key(AeadKey);

impl Key {
    #[must_use]
    pub fn new() -> Self {
        let mut key = AeadKey::default();
        thread_rng().fill_bytes(&mut key);
        Self(key)
    }

    #[must_use]
    pub fn from_slice(key: &[u8]) -> Self {
        Self(*AeadKey::from_slice(key))
    }

    #[must_use]
    pub fn from_keys(encrypt: &[u8], k: &[u8], r: &[u8]) -> Self {
        let mut key = AeadKey::default();
        key[0..32].copy_from_slice(encrypt);
        key[32..48].copy_from_slice(k);
        key[48..64].copy_from_slice(r);

        Self(key)
    }

    #[must_use]
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
    fn decrypt_data(&self, data: &[u8]) -> RusticResult<Vec<u8>> {
        if data.len() < 16 {
            return Err(CryptoErrorKind::CryptoKeyTooShort)?;
        }

        let nonce = Nonce::from_slice(&data[0..16]);
        Aes256CtrPoly1305Aes::new(&self.0)
            .decrypt(nonce, &data[16..])
            .map_err(|err| CryptoErrorKind::DataDecryptionFailed(err).into())
    }

    fn encrypt_data(&self, data: &[u8]) -> RusticResult<Vec<u8>> {
        let mut nonce = Nonce::default();
        thread_rng().fill_bytes(&mut nonce);

        let mut res = Vec::with_capacity(data.len() + 32);
        res.extend_from_slice(&nonce);
        res.extend_from_slice(data);
        let tag = Aes256CtrPoly1305Aes::new(&self.0)
            .encrypt_in_place_detached(&nonce, &[], &mut res[16..])
            .map_err(CryptoErrorKind::DataDecryptionFailed)?;
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
