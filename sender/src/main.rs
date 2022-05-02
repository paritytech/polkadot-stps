use codec::Decode;
use futures::future::try_join_all;
use log::*;
use sp_keyring::AccountKeyring;
use subxt::{
	extrinsic::Era, ClientBuilder, DefaultConfig, PairSigner, PolkadotExtrinsicParams,
	PolkadotExtrinsicParamsBuilder as Params,
};

#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod runtime {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	let url = std::env::args().skip(1).next().expect("Need node URL as argument");
	let num_ext = std::env::args()
		.skip(2)
		.next()
		.expect("Need number of extrinsics as argument")
		.parse::<u32>()
		.unwrap();
	info!("Connecting to {}", url);

	let mut signer = PairSigner::new(AccountKeyring::Alice.pair());
	let receivers = generate_receivers(num_ext);

	let api = ClientBuilder::new()
		.set_url(&url)
		.build()
		.await?
		.to_runtime_api::<runtime::RuntimeApi<DefaultConfig, PolkadotExtrinsicParams<DefaultConfig>>>(
		);

	let ext_deposit = api.constants().balances().existential_deposit().unwrap();

	// Send the transaction:
	let mut txs = Vec::new();

	info!("Signing {} transactions", num_ext);
	for i in 0..num_ext {
		//let dest = AccountKeyring::Bob.to_account_id();
		signer.set_nonce(i as u32);
		let tx_params = Params::new().era(Era::Immortal, *api.client.genesis());
		let tx = api
			.tx()
			.balances()
			.transfer(receivers[i as usize].clone().into(), ext_deposit)
			.create_signed(&signer, tx_params)
			.await?;
		txs.push(tx);
	}

	// Send the transactions in parallel:
	let mut i = 0;
	let mut last_now = std::time::Instant::now();
	let mut last_sent = 0;
	let start = std::time::Instant::now();
	const CHUNK_SIZE: usize = 50;
	info!("Sending {} transactions in chunks of {}", num_ext, CHUNK_SIZE);
	for chunk in txs.chunks(CHUNK_SIZE) {
		let mut hashes = Vec::new();
		for tx in chunk {
			let hash = api.client.rpc().submit_extrinsic(tx);
			hashes.push(hash);
		}
		try_join_all(hashes).await?;

		let elapsed = last_now.elapsed();
		if elapsed >= std::time::Duration::from_secs(1) {
			let sent = i * CHUNK_SIZE - last_sent;
			let rate = sent as f64 / elapsed.as_secs_f64();
			info!("{} txs sent in {} ms ({:.2} /s)", sent, elapsed.as_millis(), rate);
			last_now = std::time::Instant::now();
			last_sent = i * CHUNK_SIZE;
		}
		i += 1;
	}
	let rate = num_ext as f64 / start.elapsed().as_secs_f64();
	info!("{} txs sent in {} ms ({:.2} /s)", num_ext, start.elapsed().as_millis(), rate);

	Ok(())
}

fn generate_receivers(num: u32) -> Vec<subxt::sp_core::crypto::AccountId32> {
	let mut receivers = Vec::new();
	for i in 0..num {
		// Decode the account ID from the string:
		let account_id = Decode::decode(&mut &format!("{:0>32?}", i).as_bytes()[..])
			.expect("Must decode account ID");
		receivers.push(account_id);
	}
	debug!("Generated {} receiver addresses", receivers.len());
	receivers
}

#[cfg(test)]
mod tests {
	use std::collections::BTreeSet as Set;

	#[test]
	/// Check that the generated addresses are unique.
	fn generate_receivers_unique() {
		let receivers = super::generate_receivers(1024);
		let set: Set<_> = receivers.iter().collect();

		assert_eq!(set.len(), receivers.len());
	}
}
