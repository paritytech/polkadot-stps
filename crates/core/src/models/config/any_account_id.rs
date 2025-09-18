use crate::prelude::*;
use parity_scale_codec::{Decode, Encode};
use serde::Serialize;
use sp_core::{sr25519, Pair};

use subxt::{tx::Signer as _, utils::AccountId32};

#[derive(Debug, Clone, Serialize, From, PartialEq, Eq, Hash, derive_more::Display)]
pub enum AnyAccountId {
    EthereumCompat(EthAccountId),
    PolkadotBased(PolkaAccountId),
}

pub trait FromSr25519 {
    fn from_pair(value: sr25519::Pair) -> Self;
}

impl FromSr25519 for PolkaAccountId {
    fn from_pair(value: sr25519::Pair) -> Self {
        PolkaAccountId::from(value.public().0)
    }
}

impl From<AnySigner> for AnyAccountId {
    fn from(signer: AnySigner) -> Self {
        signer.account_id()
    }
}

impl From<AnyKeyPair> for AnyAccountId {
    fn from(key_pair: AnyKeyPair) -> Self {
        match key_pair {
            AnyKeyPair::PolkadotBased(p) => PolkaAccountId::from_pair(p).into(),
            AnyKeyPair::EthereumCompat(p) => EthAccountId::from_pair(p).into(),
        }
    }
}

impl Encode for AnyAccountId {
    fn encode_to<T: parity_scale_codec::Output + ?Sized>(&self, dest: &mut T) {
        match self {
            AnyAccountId::EthereumCompat(a) => a.encode_to(dest),
            AnyAccountId::PolkadotBased(a) => a.encode_to(dest),
        }
    }
}
impl Decode for AnyAccountId {
    fn decode<I: parity_scale_codec::Input>(
        input: &mut I,
    ) -> Result<Self, parity_scale_codec::Error> {
        AccountId32::decode(input)
            .map(|i| AnyAccountId::PolkadotBased(i.into()))
            .or_else(|_| EthAccountId::decode(input).map(AnyAccountId::EthereumCompat))
    }
}
