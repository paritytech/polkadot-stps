use crate::prelude::*;

use parity_scale_codec::{Decode, Encode};
use subxt::utils::MultiSignature;

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
