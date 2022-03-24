use std::fmt::Debug;

mod aespoly1305;
mod hasher;
pub use aespoly1305::*;
pub use hasher::*;

pub trait CryptoKey: Clone + Sized + Send + Sync + 'static {
    type CryptoError: Debug + Send + Sync + 'static + std::error::Error;
    fn decrypt_data(&self, data: &[u8]) -> Result<Vec<u8>, Self::CryptoError>;
    fn encrypt_data(&self, data: &[u8]) -> Result<Vec<u8>, Self::CryptoError>;
}
