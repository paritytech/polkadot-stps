use sp_core::{ecdsa, sr25519, Pair};

use crate::prelude::*;

pub fn derive_accounts(n: usize, seed: String, chain: Chain) -> Vec<AnyKeyPair> {
    match chain {
        Chain::Ethereum => derive_accounts_generic::<ecdsa::Pair>(n, seed)
            .into_iter()
            .map(AnyKeyPair::EthereumCompat)
            .collect(),
        Chain::PolkadotBased => derive_accounts_generic::<sr25519::Pair>(n, seed)
            .into_iter()
            .map(AnyKeyPair::PolkadotBased)
            .collect(),
    }
}

fn derive_accounts_generic<T>(n: usize, seed: String) -> Vec<T>
where
    T: Pair + Send + 'static,
{
    let t = std::cmp::min(
        n,
        std::thread::available_parallelism()
            .unwrap_or(1usize.try_into().unwrap())
            .get(),
    );

    let mut tn = (0..t).cycle();
    let mut tranges: Vec<_> = (0..t).map(|_| Vec::new()).collect();
    (0..n).for_each(|i| tranges[tn.next().unwrap()].push(i));
    let mut threads = Vec::new();

    tranges.into_iter().for_each(|chunk| {
        let seed = seed.clone();
        threads.push(std::thread::spawn(move || {
            chunk
                .into_iter()
                .map(move |i| {
                    let derivation = format!("{seed}{i}");
                    <T as Pair>::from_string(&derivation, None).unwrap()
                })
                .collect::<Vec<_>>()
        }));
    });

    threads
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect()
}
