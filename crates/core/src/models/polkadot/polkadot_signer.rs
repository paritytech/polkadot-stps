use sp_core::{sr25519, Pair};
use sp_runtime::{traits::Verify, MultiSignature};
use subxt::{tx::Signer, PolkadotConfig};

use crate::prelude::*;

#[derive(Clone, derive_more::Debug, Getters)]
#[debug("PolkaSigner({:?})", account_id)]
pub struct PolkaSigner {
    #[getset(get = "pub")]
    account_id: PolkaAccountId,
    #[getset(get = "pub")]
    signer: sr25519::Pair,
}
impl std::hash::Hash for PolkaSigner {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (self.account_id.as_ref().as_ref() as &[u8]).hash(state);
    }
}
impl Eq for PolkaSigner {}
impl PartialEq for PolkaSigner {
    fn eq(&self, other: &Self) -> bool {
        self.account_id == other.account_id
    }
}

impl From<sr25519::Pair> for PolkaSigner {
    fn from(pair: sr25519::Pair) -> Self {
        Self::new(pair)
    }
}

use sp_runtime::traits::IdentifyAccount;

impl PolkaSigner {
    /// Creates a new [`Signer`] from an [`sp_core::sr25519::Pair`].
    pub fn new(signer: sr25519::Pair) -> Self {
        let account_id = <MultiSignature as Verify>::Signer::from(signer.public()).into_account();
        let bytes: [u8; 32] = account_id.into();
        let account_id = PolkaAccountId::from(bytes);
        Self { account_id, signer }
    }
}

impl Signer<PolkadotConfig> for PolkaSigner {
    fn account_id(&self) -> <PolkadotConfig as subxt::Config>::AccountId {
        self.account_id.clone().as_ref().clone()
    }

    fn sign(&self, signer_payload: &[u8]) -> <PolkadotConfig as subxt::Config>::Signature {
        let signature = self.signer.sign(signer_payload);
        subxt::utils::MultiSignature::Sr25519(signature.0)
    }
}
