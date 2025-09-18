use crate::prelude::*;

use subxt::config::{
        substrate::{BlakeTwo256, SubstrateHeader},
        DefaultExtrinsicParams,
    };


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
