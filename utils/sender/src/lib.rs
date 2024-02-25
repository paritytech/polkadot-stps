use codec::Decode;
use futures::future::try_join_all;
use log::*;
use sp_core::{sr25519::Pair as SrPair, Pair};
use subxt::{
	config::extrinsic_params::{BaseExtrinsicParamsBuilder as Params, Era},
	tx::{PairSigner, SubmittableExtrinsic},
	PolkadotConfig,
	OnlineClient,
};
use utils::{runtime, Api, Error, DERIVATION};

mod pre;

pub async fn send_funds(
	api: &Api,
	sender_index: usize,
	chunk_size: usize,
	n_tx_sender: usize,
) -> Result<(), Error> {
	let receivers = generate_receivers(n_tx_sender, sender_index); // one receiver per tx
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

	info!(
		"Sender {}: sending {} transactions in chunks of {}",
		sender_index, n_tx_sender, chunk_size
	);
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

/// Generates a signer from a given index.
pub fn generate_signer(i: usize) -> PairSigner<PolkadotConfig, SrPair> {
	let pair: SrPair = Pair::from_string(format!("{}{}", DERIVATION, i).as_str(), None).unwrap();
	let signer: PairSigner<PolkadotConfig, SrPair> = PairSigner::new(pair);
	signer
}

/// Generates a vector of account IDs from a given index.
pub fn generate_receivers(n: usize, sender_index: usize) -> Vec<sp_core::crypto::AccountId32> {
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

/// Parallel version of the send funds function, except that it does not send the transactions.
/// Note that signing is a CPU bound task, and hence this cannot be async.
/// As a consequence, we use spawn_blocking here and communicate with the main thread using an unbounded channel.
pub fn parallel_signing(
	api: &Api,
	threads: &usize,
	n_tx_sender: usize,
	producer: tokio::sync::mpsc::UnboundedSender<Vec<SubmittableExtrinsic<PolkadotConfig, OnlineClient<PolkadotConfig>>>>
) -> Result<(), Error> {
	let ext_deposit_addr = runtime::constants().balances().existential_deposit();
	let genesis_hash = api.genesis_hash();
	let ext_deposit = api.constants().at(&ext_deposit_addr)?;

	for i in 0..*threads {
		let api = api.clone();
		let producer = producer.clone();
		tokio::task::spawn_blocking(move || {
			debug!("Thread {}: preparing {} transactions", i, n_tx_sender);
			let ext_deposit = ext_deposit.clone();
			let genesis_hash = genesis_hash.clone();
			let receivers = generate_receivers(n_tx_sender, i);
			let mut txs = Vec::new();
			for j in 0..n_tx_sender {
				debug!("Thread {}: preparing transaction {}", i, j);
				let shift = i * n_tx_sender;
				let signer = generate_signer(shift + j);
				debug!("Thread {}: generated signer {}{}", i, DERIVATION, shift + j);
				let tx_params = Params::new().era(Era::Immortal, genesis_hash);
				let tx_payload = runtime::tx()
					.balances()
					.transfer_keep_alive(receivers[j as usize].clone().into(), ext_deposit);
				let signed_tx =
					match api.tx().create_signed_with_nonce(&tx_payload, &signer, 0, tx_params) {
						Ok(signed) => signed,
						Err(e) => panic!("Thread {}: failed to sign transaction due to: {}", i, e),
					};
				txs.push(signed_tx);
			}
			match producer.send(txs) {
				Ok(_) => (),
				Err(e) => error!("Thread {}: failed to send transactions to consumer: {}", i, e),
			}
			info!("Thread {}: prepared and signed {} transactions", i, n_tx_sender);
		});
	}
	Ok(())
}

/// Here the signed extrinsics are submitted.
pub async fn submit_txs(
	consumer: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<SubmittableExtrinsic<PolkadotConfig, OnlineClient<PolkadotConfig>>>>,
	chunk_size: usize,
	threads: usize,
) -> Result<(), Error> {
	let mut submittable_vecs = Vec::new();
	while let Some(signed_txs) = consumer.recv().await {
		debug!("Consumer: received {} submittable transactions", signed_txs.len());
		submittable_vecs.push(signed_txs);
		if threads == submittable_vecs.len() {
			debug!("Consumer: received all submittable transactions, now starting submission");
			for vec in &submittable_vecs {
				for chunk in vec.chunks(chunk_size) {
					let mut hashes = Vec::new();
					for signed_tx in chunk {
						let hash = signed_tx.submit();
						hashes.push(hash);
					}
					try_join_all(hashes).await?;
					debug!("Sender submitted chunk with size: {}", chunk_size);
				}
			}
			info!("Sender submitted all transactions");
		}
	}
	Ok(())
}
