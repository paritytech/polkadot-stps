use crate::prelude::*;

use parity_scale_codec::{Decode, Encode};
use serde::Serialize;
use subxt::utils::AccountId32;

#[derive(
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Encode,
    Decode,
    Serialize,
    From,
    AsRef,
    derive_more::Debug,
    derive_more::Display,
)]
#[debug("{:?}", self.0)]
#[display("{}", self.0)]
#[serde(transparent)]
#[from(AccountId32, [u8; 32])]
pub struct PolkaAccountId(AccountId32);

impl std::hash::Hash for PolkaAccountId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (self.0.as_ref() as &[u8]).hash(state);
    }
}
