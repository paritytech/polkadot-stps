use crate::prelude::*;

use sp_core::{ecdsa, keccak_256};
use subxt::tx::Signer;

#[derive(Clone, derive_more::Debug, Getters)]
#[debug("EthereumSigner({})", account_id)]
pub struct EthereumSigner {
    #[getset(get = "pub")]
    account_id: EthAccountId,
    signer: ecdsa::Pair,
}
impl std::hash::Hash for EthereumSigner {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.account_id.hash(state);
    }
}
impl Eq for EthereumSigner {}
impl PartialEq for EthereumSigner {
    fn eq(&self, other: &Self) -> bool {
        self.account_id == other.account_id
    }
}

impl sp_runtime::traits::IdentifyAccount for EthereumSigner {
    type AccountId = EthAccountId;
    fn into_account(self) -> Self::AccountId {
        self.account_id
    }
}

impl From<ecdsa::Pair> for EthereumSigner {
    fn from(pair: ecdsa::Pair) -> Self {
        let account_id = EthAccountId::from(pair.clone());
        Self {
            account_id,
            signer: pair,
        }
    }
}

impl Signer<EthConfig> for EthereumSigner {
    fn account_id(&self) -> EthAccountId {
        self.account_id
    }

    fn sign(&self, signer_payload: &[u8]) -> EthSignature {
        let hash = keccak_256(signer_payload);
        let wrapped = libsecp256k1::Message::parse_slice(&hash).unwrap();
        self.signer
            .sign_prehashed(&wrapped.0.b32())
            .into()
    }
}
