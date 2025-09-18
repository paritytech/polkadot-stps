use subxt::config::{
    substrate::{BlakeTwo256, SubstrateHeader},
    DefaultExtrinsicParams,
};

use crate::prelude::*;

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
