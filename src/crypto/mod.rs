use aes256ctr_poly1305aes::{
    aead::{self, Aead, NewAead},
    Aes256CtrPoly1305Aes,
};

pub type CryptoError = aead::Error;

type Nonce = aead::Nonce<Aes256CtrPoly1305Aes>;
type AeadKey = aead::Key<Aes256CtrPoly1305Aes>;

#[derive(Clone)]
pub struct Key(AeadKey);

impl Key {
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

    pub fn decrypt_data(&self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let nonce = Nonce::from_slice(&data[0..16]);
        Aes256CtrPoly1305Aes::new(&self.0).decrypt(nonce, &data[16..])
    }
}
