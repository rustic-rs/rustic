use aes256ctr_poly1305aes::{
    aead::{self, Aead, AeadInPlace, NewAead},
    Aes256CtrPoly1305Aes,
};
use rand::{thread_rng, RngCore};

use crate::{crypto::CryptoKey, error::CryptoErrorKind, error::RusticResult};

pub(crate) type Nonce = aead::Nonce<Aes256CtrPoly1305Aes>;
pub(crate) type AeadKey = aead::Key<Aes256CtrPoly1305Aes>;

/// The `Key` is used to encrypt/MAC and check/decrypt data.
///
/// It is a 64 byte key that is used to derive the AES256 encryption key and the numbers `k` and `r` used in the `Poly1305AES` MAC.
///
/// The first 32 bytes are used for the AES256 encryption.
///
/// The next 16 bytes are used for the number `k` of `Poly1305AES`.
///
/// The last 16 bytes are used for the number `r` of `Poly1305AES`.
///
#[derive(Clone, Default, Debug, Copy)]
pub struct Key(AeadKey);

impl Key {
    /// Create a new random [`Key`] using a suitable entropy source.
    #[must_use]
    pub fn new() -> Self {
        let mut key = AeadKey::default();
        thread_rng().fill_bytes(&mut key);
        Self(key)
    }

    /// Create a new [`Key`] from a slice.
    ///
    /// # Arguments
    ///
    /// * `key` - The slice to create the [`Key`] from.
    #[must_use]
    pub fn from_slice(key: &[u8]) -> Self {
        Self(*AeadKey::from_slice(key))
    }

    /// Create a new [`Key`] from the AES key and numbers `k` and `r` for `Poly1305AES`.
    ///
    /// # Arguments
    ///
    /// * `encrypt` - The AES key.
    /// * `k` - The number k for `Poly1305AES`.
    /// * `r` - The number r for `Poly1305AES`.
    #[must_use]
    pub fn from_keys(encrypt: &[u8], k: &[u8], r: &[u8]) -> Self {
        let mut key = AeadKey::default();
        key[0..32].copy_from_slice(encrypt);
        key[32..48].copy_from_slice(k);
        key[48..64].copy_from_slice(r);

        Self(key)
    }

    /// Returns the AES key and numbers `k`and `r` for `Poly1305AES`.
    #[must_use]
    pub fn to_keys(self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
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
    /// Returns the decrypted data from the given encrypted/MACed data.
    ///
    /// # Arguments
    ///
    /// * `data` - The encrypted/MACed data.
    ///
    /// # Errors
    ///
    /// If the MAC couldn't be checked.
    fn decrypt_data(&self, data: &[u8]) -> RusticResult<Vec<u8>> {
        if data.len() < 16 {
            return Err(CryptoErrorKind::CryptoKeyTooShort)?;
        }

        let nonce = Nonce::from_slice(&data[0..16]);
        Aes256CtrPoly1305Aes::new(&self.0)
            .decrypt(nonce, &data[16..])
            .map_err(|err| CryptoErrorKind::DataDecryptionFailed(err).into())
    }

    /// Returns the encrypted+MACed data from the given data.
    ///
    /// # Arguments
    ///
    /// * `data` - The data to encrypt.
    ///
    /// # Errors
    ///
    /// If the data could not be encrypted.
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
