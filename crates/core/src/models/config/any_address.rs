use crate::prelude::*;

use parity_scale_codec::Encode;
use subxt::utils::{AccountId32, MultiAddress};

#[derive(Debug)]
pub enum AnyAddress {
    EthereumCompat(EthAccountId),
    PolkadotBased(MultiAddress<AccountId32, ()>),
}

impl From<AnyAccountId> for AnyAddress {
    fn from(account_id: AnyAccountId) -> Self {
        match account_id {
            AnyAccountId::EthereumCompat(a) => AnyAddress::EthereumCompat(a),
            AnyAccountId::PolkadotBased(a) => {
                AnyAddress::PolkadotBased(MultiAddress::from(a.as_ref().clone()))
            }
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
