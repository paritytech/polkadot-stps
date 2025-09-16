use sp_core::{ecdsa, sr25519, Pair};

use crate::prelude::*;

pub fn derive_accounts(n: usize, seed: impl AsRef<str>, chain: Chain) -> IndexSet<AnySigner> {
    derive_keys(n, seed, chain)
        .into_iter()
        .map(AnySigner::from)
        .collect()
}

fn derive_keys(n: usize, seed: impl AsRef<str>, chain: Chain) -> IndexSet<AnyKeyPair> {
    match chain {
        Chain::Ethereum => _derive_keys::<ecdsa::Pair>(n, seed)
            .into_iter()
            .map(AnyKeyPair::EthereumCompat)
            .collect(),
        Chain::PolkadotBased => _derive_keys::<sr25519::Pair>(n, seed)
            .into_iter()
            .map(AnyKeyPair::PolkadotBased)
            .collect(),
    }
}

trait DerivationFormat {
    fn derive(seed: &str, i: usize) -> String;
}

impl DerivationFormat for sp_core::sr25519::Pair {
    #[inline]
    fn derive(seed: &str, i: usize) -> String {
        format!("{seed}/{i}")
    }
}

impl DerivationFormat for sp_core::ecdsa::Pair {
    #[inline]
    fn derive(seed: &str, i: usize) -> String {
        format!("{seed}{i}")
    }
}

fn _derive_keys<T>(n: usize, seed: impl AsRef<str>) -> Vec<T>
where
    T: Pair + DerivationFormat + Send + 'static,
{
    let seed = seed.as_ref().to_string();
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
                    let derivation = <T as DerivationFormat>::derive(&seed, i);
                    let pair = <T as Pair>::from_string(&derivation, None).unwrap();
                    (i, pair)
                })
                .collect::<Vec<_>>()
        }));
    });

    let mut indexed: Vec<(usize, T)> = threads
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect();
    indexed.sort_by_key(|(i, _)| *i);
    indexed.into_iter().map(|(_, pair)| pair).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use subxt::tx::Signer;
    use test_log::test;

    fn derive_accounts_chain<'a>(
        chain: Chain,
        seed: impl AsRef<str>,
        expected: impl IntoIterator<Item = &'a str>,
    ) {
        let expected = expected
            .into_iter()
            .map(|s| s.to_string())
            .collect::<IndexSet<_>>();

        let accounts = derive_accounts(expected.len(), seed, chain)
            .into_iter()
            .map(|s| s.account_id())
            .collect::<IndexSet<_>>();

        let addresses = accounts
            .iter()
            .map(|a| a.to_string())
            .collect::<IndexSet<_>>();

        assert_eq!(addresses, expected);
    }

    #[test]
    fn derive_accounts_sr25519_sender() {
        derive_accounts_chain(
            Chain::PolkadotBased,
            "//Sender",
            [
                "5HpfmsH5yLpB27gH6SRAJdWqmN5ARrWNQAQm3LXZ6y8XG8YD",
                "5DjpL7GnqrKLKAHjzToaFeAQQfgmtmvG11XGxFaohFQbtA7z",
            ],
        );
    }

    #[test]
    fn derive_accounts_sr25519_receiver() {
        derive_accounts_chain(
            Chain::PolkadotBased,
            "//Receiver",
            [
                "5Ek9xMFVziqoS5USEUwqGgG1PNrxjD2btg5BpzoQPdaA3LXk",
                "5D5DuT9mCgynbmf4YTbUHUF83E9GWQPPven1pWMWyFUf94hQ",
            ],
        );
    }

    #[test]
    fn derive_accounts_ecdsa_sender() {
        derive_accounts_chain(
            Chain::Ethereum,
            "//Sender",
            [
                "0xb320f17a66FdBCBE3072c7E53c986dc4fd79878A",
                "0x6c55287df7A05c192CA670B1B8C9652e60402C29",
            ],
        );
    }

    #[test]
    fn derive_accounts_ecdsa_receiver() {
        derive_accounts_chain(
            Chain::Ethereum,
            "//Receiver",
            [
                "0x1Dd47683f876e0aff32A603ACC7752b121EB392C",
                "0xd5782A29D25F8B6c7bAeC712d3668DFfe2dB8eB1",
            ],
        );
    }
}
