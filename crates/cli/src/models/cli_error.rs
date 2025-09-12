use crate::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum InvalidCliArgs {
    #[error("Node url invalid {bad_value}")]
    NodeUrlInvalid { bad_value: String },
    #[error("Total senders must be positive")]
    TotalSendersMustBePositive,
    #[error("TPS cannot be zero")]
    TpsCannotBeZero,
}

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("Invalid CLI arguments")]
    InvalidCliArgs(#[from] InvalidCliArgs),

    #[error("Core error")]
    CoreError(#[from] Error),
}
