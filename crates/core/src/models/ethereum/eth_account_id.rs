use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use sha3::{Digest, Keccak256};
use sp_core::{ecdsa, keccak_256, Pair, H160};

#[derive(
    Eq, PartialEq, Copy, Clone, Encode, Decode, MaxEncodedLen, Default, PartialOrd, Ord, Hash,
)]
pub struct EthAccountId(pub [u8; 20]);

impl_serde::impl_fixed_hash_serde!(EthAccountId, 20);

impl std::fmt::Display for EthAccountId {
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

impl core::fmt::Debug for EthAccountId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", H160(self.0))
    }
}

impl EthAccountId {
    pub fn from_pair(pair: ecdsa::Pair) -> Self {
        let decompressed = libsecp256k1::PublicKey::parse_compressed(&pair.public().0)
            .expect("Wrong compressed public key provided")
            .serialize();
        let mut m = [0u8; 64];
        m.copy_from_slice(&decompressed[1..65]);
        H160::from_slice(&Keccak256::digest(m).as_slice()[12..32]).into()
    }
}

impl From<ecdsa::Pair> for EthAccountId {
    fn from(pair: ecdsa::Pair) -> Self {
        Self::from_pair(pair)
    }
}

impl From<[u8; 20]> for EthAccountId {
    fn from(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }
}

impl From<EthAccountId> for [u8; 20] {
    fn from(value: EthAccountId) -> Self {
        value.0
    }
}

impl From<H160> for EthAccountId {
    fn from(h160: H160) -> Self {
        Self(h160.0)
    }
}

impl From<EthAccountId> for H160 {
    fn from(value: EthAccountId) -> Self {
        H160(value.0)
    }
}

impl std::str::FromStr for EthAccountId {
    type Err = &'static str;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        H160::from_str(input)
            .map(Into::into)
            .map_err(|_| "invalid hex address.")
    }
}
