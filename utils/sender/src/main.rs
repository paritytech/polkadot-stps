use codec::Decode;
use futures::future::try_join_all;
use log::*;
use sp_core::{sr25519::Pair as SrPair, Pair};
use subxt::{
	tx::{BaseExtrinsicParamsBuilder as Params, Era, PairSigner},
	PolkadotConfig,
};
use clap::Parser;
use utils::{connect, runtime, Error, DERIVATION};

mod pre;

use pre::pre_conditions;

/// Util program to send transactions
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
	/// Node URL
	#[arg(long)]
	node_url: String,

	/// Sender index.
	#[arg(long)]
	sender_index: usize,

	/// Total number of senders
	#[arg(long)]
	total_senders: usize,

	/// Chunk size for sending the extrinsics.
	#[arg(long, default_value_t = 50)]
	chunk_size: usize,

	/// Total number of pre-funded accounts (on funded-accounts.json).
	#[arg(short)]
	n: usize,
}

async fn send_funds(
	node_url: &str,
	sender_index: usize,
	chunk_size: usize,
	n_tx_sender: usize,
) -> Result<(), Error> {
	let receivers = generate_receivers(n_tx_sender, sender_index); // one receiver per tx
	let api = connect(node_url).await?;

	let ext_deposit_addr = runtime::constants().balances().existential_deposit();
	let ext_deposit = api.constants().at(&ext_deposit_addr)?;

	info!("Sender {}: signing {} transactions", sender_index, n_tx_sender);
	let mut txs = Vec::new();
	for i in 0..n_tx_sender {
		let shift = sender_index * n_tx_sender;
		let signer = generate_signer(shift + i);
		let tx_params = Params::new().era(Era::Immortal, api.genesis_hash());
		let unsigned_tx = runtime::tx()
			.balances()
			.transfer_keep_alive(receivers[i as usize].clone().into(), ext_deposit);
		let signed_tx = api.tx().create_signed_with_nonce(&unsigned_tx, &signer, 0, tx_params)?;
		txs.push(signed_tx);
	}

	info!("Sender {}: sending {} transactions in chunks of {}", sender_index, n_tx_sender, chunk_size);
	let mut last_now = std::time::Instant::now();
	let mut last_sent = 0;
	let start = std::time::Instant::now();

	for (i, chunk) in txs.chunks(chunk_size).enumerate() {
		let mut hashes = Vec::new();
		for tx in chunk {
			let hash = tx.submit();
			hashes.push(hash);
		}
		try_join_all(hashes).await?;

		let elapsed = last_now.elapsed();
		if elapsed >= std::time::Duration::from_secs(1) {
			let sent = i * chunk_size - last_sent;
			let rate = sent as f64 / elapsed.as_secs_f64();
			info!(
				"Sender {}: {} txs sent in {} ms ({:.2} /s)",
				sender_index,
				sent,
				elapsed.as_millis(),
				rate
			);
			last_now = std::time::Instant::now();
			last_sent = i * chunk_size;
		}
	}
	let rate = n_tx_sender as f64 / start.elapsed().as_secs_f64();
	info!(
		"Sender {}: {} txs sent in {} ms ({:.2} /s)",
		sender_index,
		n_tx_sender,
		start.elapsed().as_millis(),
		rate
	);

	Ok(())
}

pub fn generate_signer(i: usize) -> PairSigner<PolkadotConfig, SrPair> {
	let pair: SrPair =
		Pair::from_string(format!("{}{}", DERIVATION, i).as_str(), None).unwrap();
	let signer: PairSigner<PolkadotConfig, SrPair> = PairSigner::new(pair);
	signer
}

/// Generates a vector of account IDs.
fn generate_receivers(n: usize, sender_index: usize) -> Vec<sp_core::crypto::AccountId32> {
	let shift = sender_index * n;
	let mut receivers = Vec::new();
	for i in 0..n {
		// Decode the account ID from the string:
		let account_id = Decode::decode(&mut &format!("{:0>32?}", shift + i).as_bytes()[..])
			.expect("Must decode account ID");
		receivers.push(account_id);
	}
	debug!("Generated {} receiver addresses", receivers.len());
	receivers
}

#[tokio::main]
async fn main() -> Result<(), Error> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	let args = Args::parse();

	let n_tx_sender = args.n / args.total_senders;

	pre_conditions(&args.node_url, args.sender_index, n_tx_sender).await?;

	send_funds(&args.node_url, args.sender_index, args.chunk_size, n_tx_sender).await?;
		
	Ok(())
}

#[cfg(test)]
mod tests {
	use std::collections::BTreeSet as Set;

	#[test]
	/// Check that the generated addresses are unique.
	fn generate_receivers_unique() {
		let receivers = super::generate_receivers(1024, 0);
		let set: Set<_> = receivers.iter().collect();

		assert_eq!(set.len(), receivers.len());
	}
}
