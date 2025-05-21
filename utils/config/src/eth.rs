use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use sp_core::{ecdsa, H160, keccak_256, Pair};
pub use sp_runtime::traits::IdentifyAccount;
use subxt::{
	config::Config,
	tx::Signer,
};
use sha3::{Keccak256, Digest};

#[derive(
	Eq, PartialEq, Copy, Clone, Encode, Decode, MaxEncodedLen, Default, PartialOrd, Ord, Hash
)]
pub struct AccountId20(pub [u8; 20]);

impl_serde::impl_fixed_hash_serde!(AccountId20, 20);

impl std::fmt::Display for AccountId20 {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let address = hex::encode(self.0).trim_start_matches("0x").to_lowercase();
		let address_hash = hex::encode(keccak_256(address.as_bytes()));

		let checksum: String =
			address
				.char_indices()
				.fold(String::from("0x"), |mut acc, (index, address_char)| {
					let n = u16::from_str_radix(&address_hash[index..index + 1], 16)
						.expect("Keccak256 hashed; qed");

					if n > 7 {
						// make char uppercase if ith character is 9..f
						acc.push_str(&address_char.to_uppercase().to_string())
					} else {
						// already lowercased
						acc.push(address_char)
					}

					acc
				});
		write!(f, "{checksum}")
	}
}

impl core::fmt::Debug for AccountId20 {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		write!(f, "{:?}", H160(self.0))
	}
}

impl From<[u8; 20]> for AccountId20 {
	fn from(bytes: [u8; 20]) -> Self {
		Self(bytes)
	}
}

impl From<AccountId20> for [u8; 20] {
	fn from(value: AccountId20) -> Self {
		value.0
	}
}

impl From<H160> for AccountId20 {
	fn from(h160: H160) -> Self {
		Self(h160.0)
	}
}

impl From<AccountId20> for H160 {
	fn from(value: AccountId20) -> Self {
		H160(value.0)
	}
}

impl std::str::FromStr for AccountId20 {
	type Err = &'static str;
	fn from_str(input: &str) -> Result<Self, Self::Err> {
		H160::from_str(input).map(Into::into).map_err(|_| "invalid hex address.")
	}
}

type EthSignature = [u8; 65];

pub enum MythicalConfig {}

impl Config for MythicalConfig {
    type Hash = subxt::utils::H256;
    type AccountId = AccountId20;
    type Address = AccountId20;
    type Signature = EthSignature;
    type Hasher = subxt::config::substrate::BlakeTwo256;
    type Header = subxt::config::substrate::SubstrateHeader<u32, subxt::config::substrate::BlakeTwo256>;
    type ExtrinsicParams = subxt::config::SubstrateExtrinsicParams<Self>;
    type AssetId = u32;
}

#[derive(Clone)]
pub struct EthereumSigner {
	account_id: AccountId20,
	signer: ecdsa::Pair,
}

impl sp_runtime::traits::IdentifyAccount for EthereumSigner {
	type AccountId = AccountId20;
	fn into_account(self) -> Self::AccountId {
		self.account_id
	}
}

impl From<ecdsa::Pair> for EthereumSigner {
	fn from(pair: ecdsa::Pair) -> Self {
		let decompressed = libsecp256k1::PublicKey::parse_compressed(&pair.public().0)
			.expect("Wrong compressed public key provided")
			.serialize();
		let mut m = [0u8; 64];
		m.copy_from_slice(&decompressed[1..65]);
		Self {
			account_id: H160::from_slice(&Keccak256::digest(m).as_slice()[12..32]).into(),
			signer: pair
		}
	}
}

impl From<ecdsa::Pair> for AccountId20 {
	fn from(pair: ecdsa::Pair) -> Self {
		EthereumSigner::from(pair).into_account()
	}
}

impl Signer<MythicalConfig> for EthereumSigner {
    fn account_id(&self) -> <MythicalConfig as Config>::AccountId {
        self.account_id
    }

    fn sign(&self, signer_payload: &[u8]) -> <MythicalConfig as Config>::Signature {
        let hash = keccak_256(signer_payload);
        let wrapped = libsecp256k1::Message::parse_slice(&hash).unwrap();
        self.signer.sign_prehashed(&wrapped.0.b32()).try_into().expect("Signature has correct length")
    }
}
