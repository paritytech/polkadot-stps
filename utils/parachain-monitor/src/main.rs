use std::time::Duration;

use clap::Parser;
use futures_util::StreamExt;
use log::*;
use parity_scale_codec::{Decode, Encode};
use polkadot_primitives::{
	v4::{CandidateDescriptor, CandidateReceipt},
	Hash,
};
use subxt::ext::scale_decode::DecodeAsType;
use subxt::utils::H256;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use utils::{connect, runtime, Api, Error};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
	/// Relay chain node URL
	#[arg(long)]
	validator_url: String,
	/// Para chain node node URL
	#[arg(long)]
	collator_url: String,
	/// Whether to monitor releay-chain, or para-chain finality
	#[arg(short, long, default_value_t = true)]
	para_finality: bool,
	/// Default parablock time. This will change with async-backing.
	#[arg(short, long, default_value_t = 12)]
	default_parablock_time: u64,
}

async fn count_transfers(
	para_api: &Api,
	mut rx: Receiver<H256>,
	default_parablock_time: u64,
) -> Result<(), Error> {
	let storage_timestamp_storage_addr = runtime::storage().timestamp().now();
	let mut trx_in_parablock = 0;
	while let Some(para_head) = rx.recv().await {
		info!("TPS Counter ===> Received ParaHead: {:?}", para_head);
		let parablock = para_api.blocks().at(Some(para_head)).await?;
		let parabody = parablock.body().await?;
		let parablock_number = parablock.number();
		let previous_parablock_number = parablock_number - 1;
		let maybe_previous_parablock_hash =
			para_api.rpc().block_hash(Some(previous_parablock_number.into())).await?;

		// Need to handle the case where we cannot get the previous parablock timestamp
		let parablock_time = match maybe_previous_parablock_hash {
			Some(hash) => {
				let parablock_timestamp = para_api
					.storage()
					.fetch(&storage_timestamp_storage_addr, Some(para_head))
					.await?
					.unwrap();
				let previous_parablock_timestamp = para_api
					.storage()
					.fetch(&storage_timestamp_storage_addr, Some(hash))
					.await?
					.unwrap();
				let time_diff = parablock_timestamp - previous_parablock_timestamp;
				info!("TPS Counter ===> Parablock time estimated at: {:?}", time_diff);
				time_diff
			},
			// Assume default if unable to get the previous parablock from parablock number
			None => {
				warn!(
					"TPS Counter ===> Assuming default parablock time of: {:?}",
					default_parablock_time
				);
				Duration::as_secs_f64(&Duration::new(default_parablock_time, 0)) as u64
			},
		};
		for extrinsic in parabody.extrinsics() {
			for events in extrinsic.events().await {
				for event in events.iter() {
					let evt = event?;
					let variant = evt.variant_name();
					if variant == "Transfer" {
						trx_in_parablock += 1;
					}
				}
			}
		}
		// Print to stdout and reset counter
		info!(
			"TPS Counter ===> Counted {} TPS in ParaHead: {:?}",
			trx_in_parablock / parablock_time,
			para_head
		);
	}
	trx_in_parablock = 0;
	Ok(())
}

async fn subscribe(relay_api: &Api, tx: Sender<H256>) -> Result<(), Error> {
	// Subscribe to all finalized blocks:
	let mut blocks_sub = relay_api.blocks().subscribe_finalized().await?;
	// For each block, check if the CandidateIncluded extrinsic occurs
	while let Some(block) = blocks_sub.next().await {
		let block = block?;
		let body = block.body().await?;
		for ext in body.extrinsics() {
			let events = ext.events().await?;
			for evt in events.iter() {
				let evt = evt?;
				let event_name = evt.variant_name();
				if event_name == "CandidateIncluded" {
					// If the CandidateIncluded event occurs, we need to get the CandidateDescriptor by decoding bytes
					let mut values = evt.field_bytes();
					let candidate_receipt = CandidateReceipt::<Hash>::decode(&mut values)?;
					let descriptor = candidate_receipt.descriptor();
					info!(
						"ParaBlock Subscriber ===> New ParaHead: {:?} for ParaId: {:?}",
						descriptor.para_head, descriptor.para_id
					);
					tx.send(descriptor.para_head).await?;
				}
			}
		}
	}
	Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	// Don't need to process many messages concurrently as throughput depends on parablock times
	let (tx, mut rx) = channel::<H256>(10);

	let args = Args::parse();
	let validator_url = args.validator_url;
	let collator_url = args.collator_url;
	let relay_api = connect(&validator_url).await?;
	let para_api = connect(&collator_url).await?;

	info!("Spawning new async thread for subscribing to relay chain blocks.");
	tokio::spawn(async move {
		subscribe(&relay_api, tx).await;
	});

	info!("Counting Transfer events");
	count_transfers(&para_api, rx, args.default_parablock_time).await?;

	Ok(())
}
