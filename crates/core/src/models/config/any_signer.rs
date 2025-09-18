use subxt::tx::Signer;

use crate::prelude::*;

#[derive(Clone, Debug, From, PartialEq, Eq, Hash)]
pub enum AnySigner {
    PolkadotBased(Box<PolkaSigner>),
    EthereumCompat(EthereumSigner),
}

impl From<&AnySigner> for StorageAddress {
    fn from(value: &AnySigner) -> Self {
        use subxt::tx::Signer;
        Self::from(value.account_id())
    }
}

impl From<AnyKeyPair> for AnySigner {
    fn from(key_pair: AnyKeyPair) -> Self {
        match key_pair {
            AnyKeyPair::PolkadotBased(p) => Box::new(PolkaSigner::from(p)).into(),
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
            AnySigner::PolkadotBased(s) => s.sign(payload).into(),
            AnySigner::EthereumCompat(s) => s.sign(payload).into(),
        }
    }
}
