use crate::RusticResult;
use secrecy::{CloneableSecret, DebugSecret, Secret, Zeroize};

pub(crate) mod aespoly1305;
pub(crate) mod hasher;

pub trait CryptoKey: Clone + Sized + Send + Sync + 'static {
    fn decrypt_data(&self, data: &[u8]) -> RusticResult<Vec<u8>>;
    fn encrypt_data(&self, data: &[u8]) -> RusticResult<Vec<u8>>;
}

#[derive(Clone, Debug, Default)]
pub struct RusticPassword(String);

impl std::ops::Deref for RusticPassword {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl RusticPassword {
    pub fn new(password: String) -> Self {
        Self(password)
    }
}

impl Zeroize for RusticPassword {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl DebugSecret for RusticPassword {}
impl CloneableSecret for RusticPassword {}

/// Our Secret Password
pub type SecretPassword = Secret<RusticPassword>;
