use clap::Parser;
use clap::ArgAction::Set;
use log::*;
use tokio::time::{sleep, Duration};
use utils::{connect, runtime, Api, Error};
use futures_util::StreamExt;
use parity_scale_codec::{Decode, Encode};
use polkadot_primitives::{
	v4::{CandidateDescriptor, CandidateReceipt},
	Hash,
};
use subxt::ext::scale_decode::DecodeAsType;
use subxt::utils::H256;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{channel, Receiver, Sender};

/// util program to count TPS
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
	/// Node URL
	#[arg(long)]
	node_url: Option<String>,

	/// Relay node URL
	#[arg(long)]
	validator_url: Option<String>,

	/// Collator node URL
	#[arg(long)]
	collator_url: Option<String>,

	/// Total number of senders
	#[arg(long)]
	total_senders: usize,

	/// Total number of expected transactions
	#[arg(long)]
	num: usize,

	/// Whether to subscribe to blocks from genesis or not.
	/// For zombienet tests, this should be set to true.
	/// When deploying tps in more long-living networks, set this to false (or simply omit it).
	#[arg(short, long)]
	genesis: bool,

	/// Whether to monitor relay-chain, or para-chain finality.
	/// If set to true, tps will subscribe to CandidateIncluded events on the relaychain node,
	/// and scrape Balances Transfer events concurrently with a collator node RPC client.
	#[arg(short, long)]
	para_finality: bool,

	/// Default parablock time set to 12s for sync-backing.
	/// This should be set to 6.0s for async-backing.
	#[arg(short, long, default_value_t = 12)]
	default_parablock_time: u64
}

/// in case we're monitoring TPS on a parachain
/// we spawn a thread to subscribe for CandidateIncluded events coming from the relay chain
/// so we can signal to the calc_para_tps thread which finalized block to scrape
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

/// in case we're monitoring TPS on a parachain
/// calc_para_tps thread listens for which parachain block to scrape
async fn calc_para_tps(
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

/// in case we're monitoring TPS on relay chain
/// we simply loop over blocks and count the number of transfers in each block
/// we take the timestamp difference between block X and block X-1
pub async fn calc_relay_tps(api: &Api, n: usize, genesis: bool) -> Result<(), Error> {
	let storage_timestamp_storage_addr = runtime::storage().timestamp().now();
	// do we start from genesis, or latest finalized block?
	let (mut block_number_x, mut block_timestamp_x_minus_one) = match genesis {
		true => {
			let block_hash_genesis = api
				.rpc()
				.block_hash(Some(1u32.into()))
				.await?
				.expect("genesis exists, therefore hash exists; qed");
			let block_timestamp_genesis = api
				.storage()
				.fetch(&storage_timestamp_storage_addr, Some(block_hash_genesis))
				.await?
				.unwrap();
			(2, block_timestamp_genesis)
		},
		false => {
			let block_hash_x = api.rpc().finalized_head().await?;
			let block_header_x = api
				.rpc()
				.header(Some(block_hash_x))
				.await?
				.expect("hash exists, therefore header exists; qed");
			let block_number_x = block_header_x.number;
			let block_hash_x_minus_one = api
				.rpc()
				.block_hash(Some((block_number_x - 1u32).into()))
				.await?
				.expect("block number exists, therefore hash exists; qed");
			let block_timestamp_x_minus_one = api
				.storage()
				.fetch(&storage_timestamp_storage_addr, Some(block_hash_x_minus_one))
				.await?
				.unwrap();
			(block_number_x, block_timestamp_x_minus_one)
		},
	};

	let mut total_count = 0;
	let mut tps_vec = Vec::new();

	loop {
		let mut block_hash_x = api.rpc().block_hash(Some(block_number_x.into())).await?;
		while block_hash_x.is_none() {
			info!("waiting for finalization of block {}", block_number_x);
			sleep(Duration::from_secs(6)).await;

			block_hash_x = api.rpc().block_hash(Some(block_number_x.into())).await?;
		}

		let block_timestamp_x =
			api.storage().fetch(&storage_timestamp_storage_addr, block_hash_x).await?.unwrap();
		let time_diff = block_timestamp_x - block_timestamp_x_minus_one;
		block_timestamp_x_minus_one = block_timestamp_x;

		let mut tps_count = 0;
		let events = api.events().at(block_hash_x).await?;
		for event in events.iter().flatten() {
			if event.pallet_name() == "Balances" && event.variant_name() == "Transfer" {
				total_count += 1;
				tps_count += 1;
			}
		}

		if tps_count > 0 {
			let tps = tps_count as f32 / (time_diff as f32 / 1000.0);
			tps_vec.push(tps);
			info!("TPS on block {}: {}", block_number_x, tps);
		}

		block_number_x += 1;
		if total_count >= n {
			let avg_tps: f32 = tps_vec.iter().sum::<f32>() / tps_vec.len() as f32;
			info!("average TPS: {}", avg_tps);
			break;
		}
	}

	Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	let args = Args::parse();
	let para_finality = args.para_finality;

	match para_finality {
		true => {
			// Don't need to process many messages concurrently as throughput depends on parablock times
			let (tx, mut rx) = channel::<H256>(10);
			let validator_url = match args.validator_url {
				Some(s) => s,
				None => panic!("Must pass --validator-url when setting --para-finality to 'true'")
			};
			let collator_url = match args.collator_url {
				Some(s) => s,
				None => panic!("Must pass --collator-url when setting --para-finality to 'true'")
			};
			let relay_api = connect(&validator_url).await?;
			let para_api = connect(&collator_url).await?;

			info!("Spawning new async thread for subscribing to relay chain blocks.");
			tokio::spawn(async move {
				subscribe(&relay_api, tx).await;
			});

			info!("Counting Transfer events from async main thread");
			calc_para_tps(&para_api, rx, args.default_parablock_time).await?;
		},
		
		false => {
			let node_url = match args.node_url {
				Some(s) => s,
				None => panic!("Must pass --node-url when setting --para-finality to 'false'")
			};
			let n_tx_truncated = (args.num / args.total_senders) * args.total_senders;
			let api = connect(&node_url).await?;
			calc_relay_tps(&api, n_tx_truncated, args.genesis).await?;
		}
	}

	Ok(())
}
