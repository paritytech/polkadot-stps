use crate::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Bootstrap error {0}")]
    Bootstrap(#[from] BootstrapSpammerError),

    #[error("Get nonce error {0}")]
    GetNonceError(#[from] GetNonceError),
}

#[derive(Debug, thiserror::Error)]
pub enum BootstrapSpammerError {
    #[error("Failed to create API client: {underlying}")]
    CreateApiFailure { underlying: String },
}
