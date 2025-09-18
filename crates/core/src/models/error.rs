use crate::prelude::*;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Bootstrap error {0}")]
    Bootstrap(#[from] BootstrapSpammerError),

    #[error("Get nonce error {0}")]
    GetNonceError(#[from] Box<GetNonceError>),

    #[error("Join senders error {0}")]
    JoinSendersError(#[from] Box<tokio::task::JoinError>),
}

#[derive(Debug, thiserror::Error)]
pub enum BootstrapSpammerError {
    #[error("Failed to create API client: {underlying}")]
    CreateApiFailure { underlying: String },
}
