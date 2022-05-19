use subxt::{
	sp_core::{sr25519::Pair as SrPair, Pair},
	DefaultConfig, PairSigner,
};
use serde_json::Value;
use std::{fs::File, path::PathBuf, io::{Read, Write}};

/// Initial funds for a genesis account.
const FUNDS: u64 = 10_000_000_000_000_000;

pub fn funded_accounts_json(derivation_blueprint: &str, n: usize, json_path: PathBuf) {
	let mut v = Vec::new();
	for i in 0..n {
		let pair: SrPair =
			Pair::from_string(format!("{}{}", derivation_blueprint, i).as_str(), None).unwrap();
		let signer: PairSigner<DefaultConfig, SrPair> = PairSigner::new(pair);
		let a: (String, u64) = (signer.account_id().to_string(), FUNDS);
		v.push(a);
	}

	let v_json = serde_json::to_value(&v).unwrap();
	let json_bytes = serde_json::to_vec_pretty(&v_json).unwrap();

	let mut file = File::create(json_path).unwrap();
	file.write_all(&json_bytes).unwrap();
}

pub fn n_accounts(json_path: &PathBuf) -> usize {
	let mut file = File::open(json_path).unwrap();
	let mut json_bytes = Vec::new();
	file.read_to_end(&mut json_bytes).expect("Unable to read data");

	let json: Value = serde_json::from_slice(&json_bytes).unwrap();
	let json_array = json.as_array().unwrap();
	json_array.len()
}