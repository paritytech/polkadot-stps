use crate::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unknown error")]
    Unknown,
}
