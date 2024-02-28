use codec::Decode;
use futures::future::try_join_all;
use log::*;
use sp_core::{sr25519::Pair as SrPair, ByteArray, Pair};
use subxt::{
	config::extrinsic_params::{BaseExtrinsicParamsBuilder as Params, Era},
	dynamic::Value,
	tx::{PairSigner, SubmittableExtrinsic},
	OnlineClient, PolkadotConfig,
};
use utils::{Api, Error, DERIVATION};

mod pre;

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
	threads: usize,
	n_tx_sender: usize,
	producer: tokio::sync::mpsc::UnboundedSender<
		Vec<SubmittableExtrinsic<PolkadotConfig, OnlineClient<PolkadotConfig>>>,
	>,
) -> Result<(), Error> {
	let genesis_hash = api.genesis_hash();
	let ext_deposit_query = subxt::dynamic::constant("Balances", "ExistentialDeposit");
	let ext_deposit =
		u128::decode(&mut &api.constants().at(&ext_deposit_query)?.into_encoded()[..])?;

	for i in 0..threads {
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
				let tx_call = subxt::dynamic::tx(
					"Balances",
					"transfer_keep_alive",
					vec![
						Value::unnamed_variant("Id", [Value::from_bytes(receivers[j].as_slice())]),
						Value::u128(ext_deposit),
					],
				);
				let signed_tx =
					match api.tx().create_signed_with_nonce(&tx_call, &signer, 0, tx_params) {
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
	consumer: &mut tokio::sync::mpsc::UnboundedReceiver<
		Vec<SubmittableExtrinsic<PolkadotConfig, OnlineClient<PolkadotConfig>>>,
	>,
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
