use clap::Parser;
use codec::Decode;
use futures::future::try_join_all;
use log::*;
use sp_keyring::AccountKeyring;
use subxt::{
	extrinsic::Era, ClientBuilder, DefaultConfig, PairSigner, PolkadotExtrinsicParams,
	PolkadotExtrinsicParamsBuilder as Params,
};

/// Send many extrinsics to a node.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
	/// The node to connect to.
	#[clap(long, short)]
	node: String,

	/// Number of extrinsics to send.
	#[clap(long, short)]
	extrinsics: usize,

	/// Chunk size for sending the extrinsics.
	#[clap(long, short, default_value_t = 50)]
	chunk_size: usize,
}

#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod runtime {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	let args = Args::parse();
	let mut signer = PairSigner::new(AccountKeyring::Alice.pair());
	let receivers = generate_receivers(args.extrinsics);

	let api = ClientBuilder::new()
		.set_url(&args.node)
		.build()
		.await?
		.to_runtime_api::<runtime::RuntimeApi<DefaultConfig, PolkadotExtrinsicParams<DefaultConfig>>>(
		);

	let ext_deposit = api.constants().balances().existential_deposit().unwrap();

	info!("Signing {} transactions", args.extrinsics);
	let mut txs = Vec::new();
	for i in 0..args.extrinsics {
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

	info!("Sending {} transactions in chunks of {}", args.extrinsics, args.chunk_size);
	let mut i = 0;
	let mut last_now = std::time::Instant::now();
	let mut last_sent = 0;
	let start = std::time::Instant::now();

	for chunk in txs.chunks(args.chunk_size) {
		let mut hashes = Vec::new();
		for tx in chunk {
			let hash = api.client.rpc().submit_extrinsic(tx);
			hashes.push(hash);
		}
		try_join_all(hashes).await?;

		let elapsed = last_now.elapsed();
		if elapsed >= std::time::Duration::from_secs(1) {
			let sent = i * args.chunk_size - last_sent;
			let rate = sent as f64 / elapsed.as_secs_f64();
			info!("{} txs sent in {} ms ({:.2} /s)", sent, elapsed.as_millis(), rate);
			last_now = std::time::Instant::now();
			last_sent = i * args.chunk_size;
		}
		i += 1;
	}
	let rate = args.extrinsics as f64 / start.elapsed().as_secs_f64();
	info!("{} txs sent in {} ms ({:.2} /s)", args.extrinsics, start.elapsed().as_millis(), rate);

	Ok(())
}

/// Generates a vector of account IDs.
fn generate_receivers(num: usize) -> Vec<subxt::sp_core::crypto::AccountId32> {
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
