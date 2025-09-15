use parity_scale_codec::{Decode, Encode};
use serde::Serialize;
use sp_core::{ecdsa, sr25519, Pair};
use subxt::{
    config::{
        substrate::{BlakeTwo256, SubstrateHeader},
        DefaultExtrinsicParams,
    },
    tx::Signer,
    utils::{AccountId32, MultiAddress, MultiSignature},
};

use crate::prelude::*;

#[derive(Clone)]
pub enum AnyKeyPair {
    EthereumCompat(ecdsa::Pair),
    PolkadotBased(sr25519::Pair),
}
impl std::hash::Hash for AnyKeyPair {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            AnyKeyPair::EthereumCompat(a) => a.public().0.hash(state),
            AnyKeyPair::PolkadotBased(a) => a.public().0.hash(state),
        }
    }
}
impl Eq for AnyKeyPair {}
impl PartialEq for AnyKeyPair {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AnyKeyPair::EthereumCompat(a), AnyKeyPair::EthereumCompat(b)) => {
                a.public() == b.public()
            }
            (AnyKeyPair::PolkadotBased(a), AnyKeyPair::PolkadotBased(b)) => {
                a.public() == b.public()
            }
            _ => false,
        }
    }
}

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
)]
#[debug("{}", self.0)]
#[serde(transparent)]
#[from(AccountId32, [u8; 32])]
pub struct PolkaAccountId(AccountId32);

impl std::hash::Hash for PolkaAccountId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (self.0.as_ref() as &[u8]).hash(state);
    }
}

#[derive(Clone, Debug, From, PartialEq, Eq, Hash)]
pub enum AnySigner {
    PolkadotBased(PolkaSigner),
    EthereumCompat(EthereumSigner),
}

impl From<AnyKeyPair> for AnySigner {
    fn from(key_pair: AnyKeyPair) -> Self {
        match key_pair {
            AnyKeyPair::PolkadotBased(p) => PolkaSigner::from(p).into(),
            AnyKeyPair::EthereumCompat(p) => EthereumSigner::from(p).into(),
        }
    }
}

impl Signer<AnyConfig> for AnySigner {
    fn account_id(&self) -> AnyAccountId {
        match self {
            AnySigner::PolkadotBased(s) => AnyAccountId::PolkadotBased(s.account_id().clone()),
            AnySigner::EthereumCompat(s) => AnyAccountId::EthereumCompat(*s.account_id()),
        }
    }

    fn sign(&self, payload: &[u8]) -> AnySignature {
        match self {
            AnySigner::PolkadotBased(s) => {
                todo!()
            }
            AnySigner::EthereumCompat(s) => {
                todo!()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, From, PartialEq, Eq, Hash)]
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

#[derive(Debug)]
pub enum AnyAddress {
    EthereumCompat(EthAccountId),
    PolkadotBased(MultiAddress<AccountId32, ()>),
}

impl From<AnyAccountId> for AnyAddress {
    fn from(account_id: AnyAccountId) -> Self {
        match account_id {
            AnyAccountId::EthereumCompat(a) => AnyAddress::EthereumCompat(a),
            AnyAccountId::PolkadotBased(a) => AnyAddress::PolkadotBased(MultiAddress::from(a.0)),
        }
    }
}

impl Encode for AnyAddress {
    fn encode_to<T: parity_scale_codec::Output + ?Sized>(&self, dest: &mut T) {
        match self {
            AnyAddress::EthereumCompat(a) => a.encode_to(dest),
            AnyAddress::PolkadotBased(a) => a.encode_to(dest),
        }
    }
}

pub type EthSignature = [u8; 65];

#[derive(Debug, Clone, From)]
pub enum AnySignature {
    PolkadotBased(MultiSignature),
    EthereumCompat(EthSignature),
}
impl Encode for AnySignature {
    fn encode_to<T: parity_scale_codec::Output + ?Sized>(&self, dest: &mut T) {
        match self {
            AnySignature::PolkadotBased(a) => a.encode_to(dest),
            AnySignature::EthereumCompat(a) => a.encode_to(dest),
        }
    }
}
impl Decode for AnySignature {
    fn decode<I: parity_scale_codec::Input>(
        input: &mut I,
    ) -> Result<Self, parity_scale_codec::Error> {
        MultiSignature::decode(input)
            .map(AnySignature::PolkadotBased)
            .or_else(|_| EthSignature::decode(input).map(AnySignature::EthereumCompat))
    }
}

pub struct AnyConfig;
impl subxt::Config for AnyConfig {
    type Hasher = BlakeTwo256;
    type Header = SubstrateHeader<u32, BlakeTwo256>;
    type AssetId = u32;
    type ExtrinsicParams = DefaultExtrinsicParams<Self>;

    type AccountId = AnyAccountId;
    type Address = AnyAddress;
    type Signature = AnySignature;
}

pub struct EthConfig;
impl subxt::Config for EthConfig {
    type Hasher = BlakeTwo256;
    type Header = SubstrateHeader<u32, BlakeTwo256>;
    type AssetId = u32;
    type ExtrinsicParams = DefaultExtrinsicParams<Self>;

    type AccountId = EthAccountId;
    type Address = EthAccountId;

    type Signature = EthSignature;
}
