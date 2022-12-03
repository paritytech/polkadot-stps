use codec::Decode;
use futures::future::try_join_all;
use log::*;
use sp_core::{sr25519::Pair as SrPair, Pair};
use subxt::{
	tx::{BaseExtrinsicParamsBuilder as Params, Era, PairSigner},
	SubstrateConfig,
};

use crate::shared::{connect, runtime, Error};

async fn wait_for_events(node: String, node_index: usize, n: usize) -> Result<(), Error> {
	let api = connect(&node).await?;

	let mut balance_transfer_count = 0;
	let mut last_checked_block_number = 0;

	let mut finalized_block_headers = api.rpc().subscribe_finalized_block_headers().await?;

	while let Some(b) = finalized_block_headers.next().await {
		let finalized_block_header = b.unwrap();
		let finalized_block_number = finalized_block_header.number;

		for i in last_checked_block_number..finalized_block_number {
			let block_hash = api.rpc().block_hash(Some(i.into())).await?;

			let events = api.events().at(block_hash).await?;
			for event in events.iter().flatten() {
				if event.pallet_name() == "Balances" && event.variant_name() == "Transfer" {
					balance_transfer_count += 1;
				}
			}
		}

		last_checked_block_number = finalized_block_number;

		if balance_transfer_count >= n {
			info!("Node {}: Found all {} transfer events", node_index, balance_transfer_count);
			break;
		}
		if balance_transfer_count > 0 {
			info!(
				"Node {}: Found {} transfer events, need {} more",
				node_index,
				balance_transfer_count,
				n - balance_transfer_count
			);
		}
	}

	Ok(())
}

pub async fn send_funds(
	node: String,
	node_index: usize,
	derivation: &str,
	chunk_size: usize,
	n_tx_sender: usize,
	n_accounts_truncated: usize,
) -> Result<(), Error> {
	let receivers = generate_receivers(n_tx_sender, node_index); // one receiver per sender
	let api = connect(&node).await?;

	let ext_deposit_addr = runtime::constants().balances().existential_deposit();
	let ext_deposit = api.constants().at(&ext_deposit_addr)?;

	info!("Node {}: signing {} transactions", node_index, n_tx_sender);
	let mut txs = Vec::new();
	for i in 0..n_tx_sender {
		let shift = node_index * n_tx_sender;
		let signer = generate_signer(derivation, shift + i);
		let tx_params = Params::new().era(Era::Immortal, api.genesis_hash());
		let unsigned_tx = runtime::tx()
			.balances()
			.transfer_keep_alive(receivers[i as usize].clone().into(), ext_deposit);
		let signed_tx = api.tx().create_signed(&unsigned_tx, &signer, tx_params).await?;
		txs.push(signed_tx);
	}

	// Start a second thread to listen for `Transfer` events.
	let wait_for_events =
		tokio::task::spawn(
			async move { wait_for_events(node, node_index, n_accounts_truncated).await },
		);

	info!("Node {}: sending {} transactions in chunks of {}", node_index, n_tx_sender, chunk_size);
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
				"Node {}: {} txs sent in {} ms ({:.2} /s)",
				node_index,
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
		"Node {}: {} txs sent in {} ms ({:.2} /s)",
		node_index,
		n_tx_sender,
		start.elapsed().as_millis(),
		rate
	);

	// Wait until all `Transfer` events were received.
	// Any timeout can be handled by the Zombienet DSL.
	wait_for_events
		.await?
		.map_err(|e| format!("Failed to wait for events: {:?}", e))?;
	Ok(())
}

pub fn generate_signer(
	derivation_blueprint: &str,
	i: usize,
) -> PairSigner<SubstrateConfig, SrPair> {
	let pair: SrPair =
		Pair::from_string(format!("{}{}", derivation_blueprint, i).as_str(), None).unwrap();
	let signer: PairSigner<SubstrateConfig, SrPair> = PairSigner::new(pair);
	signer
}

/// Generates a vector of account IDs.
fn generate_receivers(n: usize, node_index: usize) -> Vec<sp_core::crypto::AccountId32> {
	let shift = node_index * n;
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
