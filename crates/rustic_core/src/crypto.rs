use crate::RusticResult;

pub(crate) mod aespoly1305;
pub(crate) mod hasher;

pub trait CryptoKey: Clone + Sized + Send + Sync + 'static {
    fn decrypt_data(&self, data: &[u8]) -> RusticResult<Vec<u8>>;
    fn encrypt_data(&self, data: &[u8]) -> RusticResult<Vec<u8>>;
}
