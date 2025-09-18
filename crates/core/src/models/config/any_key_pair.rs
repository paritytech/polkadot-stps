use sp_core::{ecdsa, sr25519, Pair};



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