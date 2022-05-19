use subxt::{
	sp_core::{sr25519::Pair as SrPair, Pair},
	DefaultConfig, PairSigner,
};

/// Initial funds for a genesis account.
const FUNDS: u64 = 10_000_000_000_000_000;

pub fn funded_accounts_json(derivation_blueprint: &str, n: usize) -> Vec<u8> {
	let mut v = Vec::new();
	for i in 0..n {
		let pair: SrPair =
			Pair::from_string(format!("{}{}", derivation_blueprint, i).as_str(), None).unwrap();
		let signer: PairSigner<DefaultConfig, SrPair> = PairSigner::new(pair);
		let a: (String, u64) = (signer.account_id().to_string(), FUNDS);
		v.push(a);
	}

	let v_json = serde_json::to_value(&v).unwrap();
	serde_json::to_vec_pretty(&v_json).unwrap()
}
