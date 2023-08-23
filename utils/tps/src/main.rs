use clap::Parser;
use futures_util::StreamExt;
use log::*;
use parity_scale_codec::Decode;
use polkadot_primitives::{CandidateReceipt, Hash, Id};
use subxt::utils::H256;
use tokio::sync::{
	mpsc::{channel, Receiver, Sender},
	oneshot::Sender as OneshotSender,
	oneshot::Receiver as OneshotReceiver,
	oneshot::channel as oneshot_channel,
};
use tokio::time::Duration;
use utils::{connect, runtime, Api, Error};

mod prometheus;
use crate::prometheus::*;

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

	/// When para_finality is set, we need to be explicit about which parachain to follow.
	/// Also, make sure the collator_url is set to a collator on the same parachain accordingly.
	#[arg(long)]
	para_id: Option<u32>,

	/// Default parablock time set to 12s for sync-backing.
	/// This should be set to 6.0s for async-backing.
	#[arg(short, long, default_value_t = 6)]
	default_parablock_time: u64,

	/// Whether to export metrics to prometheus
	#[arg(long)]
	prometheus: bool,

	/// Prometheus Listener URL
	#[arg(long, default_value = "0.0.0.0")]
	prometheus_url: String,

	/// Prometheus Listener Port
	#[arg(long, default_value_t = 65432)]
	prometheus_port: u16,
}

/// In case we're monitoring TPS on a parachain this function subscribes to parablocks,
/// and awaits a signal from main thread to stop execution.
async fn subscribe(
	relay_api: Api,
	para_id: u32,
	mut signal_receiver: OneshotReceiver<bool>,
	parablock_hash_sender: Sender<H256>,
) -> Result<(), Error> {
	let mut blocks_sub = relay_api.blocks().subscribe_finalized().await?;
	while let Some(block) = blocks_sub.next().await {
		let block = block?;
		let body = block.body().await?;
		let extrinsics = body.extrinsics();
		for ext in extrinsics.iter() {
			let ext = ext?;
			let events = ext.events().await?;
			for evt in events.iter() {
				let evt = evt?;
				let event_name = evt.variant_name();
				if event_name == "CandidateIncluded" {
					let mut values = evt.field_bytes();
					let candidate_receipt = CandidateReceipt::<Hash>::decode(&mut values)?;
					let descriptor = candidate_receipt.descriptor();
					let parablock_para_id = descriptor.para_id;
					if parablock_para_id.eq(&Id::new(para_id)) {
						info!(
							"New ParaHead: {:?} for ParaId: {:?}",
							descriptor.para_head, parablock_para_id
						);
						parablock_hash_sender.send(descriptor.para_head).await?;
					} else {
						info!(
							"New ParaHead: {:?} for ParaId: {:?} which we are not calculating (s)TPS for",
							descriptor.para_head, parablock_para_id
						);
					}
				}
			}
		}
		// If we received a signal to stop execution, break the loop
		match signal_receiver.try_recv() {
			Ok(_) => {
				info!("Worker stop signal received, now stopping execution!");
				break;
			},
			Err(err) => {
				trace!("Tried to receive on oneshot channel: {:?}", err);
				continue;
			},
		}
	}
	Ok(())
}

/// In case we're monitoring TPS on a parachain this function subscribes to a parablock hash,
/// and calculates the transfer events within the parablock as well as the parablock time.
/// Once the number of expected transactions is calculated, it signals to the subscriber thread to stop execution.
async fn calc_para_tps(
	para_api: Api,
	mut parablock_hash_receiver: Receiver<H256>,
	signal_sender: OneshotSender<bool>,
	default_parablock_time: u64,
	expected_transactions: usize,
	prometheus_metrics: Option<StpsMetrics>,
) -> Result<(), Error> {
	let storage_timestamp_storage_addr = runtime::storage().timestamp().now();
	let mut trx_in_parablock = 0;
	let mut total_count = 0;
	let mut tps_vec = Vec::new();

	while let Some(para_head) = parablock_hash_receiver.recv().await {
		info!("Received ParaHead: {:?}", para_head);
		let parablock = para_api.blocks().at(para_head).await?;
		let parabody = parablock.body().await?;
		let parablock_hash = parablock.hash();
		let parablock_number = parablock.number();

		// Skip the first parablock as no way to calculate time-difference between it and non-existing block 0
		if parablock_number == 1 {
			info!("Received Parablock number: {:?}, skipping accordingly.", parablock_number);
			continue;
		}

		let previous_parablock_number = parablock_number - 1;
		let maybe_previous_parablock_hash =
			para_api.rpc().block_hash(Some(previous_parablock_number.into())).await?;

		let parablock_time = match maybe_previous_parablock_hash {
			Some(previous_hash) => {
				let parablock_timestamp = para_api
					.storage()
					.at(parablock_hash)
					.fetch(&storage_timestamp_storage_addr)
					.await?
					.unwrap();
				let previous_parablock_timestamp = para_api
					.storage()
					.at(previous_hash)
					.fetch(&storage_timestamp_storage_addr)
					.await?
					.unwrap();
				let time_diff = parablock_timestamp - previous_parablock_timestamp;
				info!("Parablock time estimated at: {:?}ms", time_diff);
				time_diff
			},
			None => {
				warn!(
					"Unable to calculate parablock time. Assuming default parablock time of: {:?}s",
					default_parablock_time
				);
				Duration::as_secs_f64(&Duration::new(default_parablock_time, 0)) as u64
			},
		};
		info!("Checking extrinsics in parablock: {:?}", parablock_number);
		for extrinsic in parabody.extrinsics().iter() {
			let extrinsic = extrinsic?;
			let events = extrinsic.events().await?;
			for event in events.iter() {
				let evt = event?;
				let variant = evt.variant_name();
				if variant == "Transfer" {
					trx_in_parablock += 1;
					total_count += 1;
				}
			}
		}
		info!("Found {:?} transfers in parablock: {:?}", trx_in_parablock, parablock_number);
		if trx_in_parablock > 0 {
			let tps = trx_in_parablock as f32 / (parablock_time as f32 / 1000.0);
			tps_vec.push(tps);
			info!("TPS on parablock {}: {}", parablock_number, tps);
			info!("Total transactions processed: {}", total_count);
			info!("Remaining transactions to process: {}", expected_transactions - total_count);
		}

		// Send metrics to prometheus, if enabled
		if let Some(metrics) = &prometheus_metrics {
			metrics.set(trx_in_parablock, parablock_time, parablock_number);
		}

		// Reset counter
		trx_in_parablock = 0;

		// Check whether to exit the loop
		if total_count >= expected_transactions {
			let avg_tps: f32 = tps_vec.iter().sum::<f32>() / tps_vec.len() as f32;
			info!("Average TPS is estimated as: {}", avg_tps);
			match signal_sender.send(true) {
				Ok(_) => {
					info!("Signaled to stop execution!");
				},
				Err(_) => {
					panic!("Failed to send signal to stop execution, exiting!");
				},
			}
			break
		}
	}
	Ok(())
}

/// in case we're monitoring TPS on relay chain
/// we simply loop over blocks and count the number of transfers in each block
/// we take the timestamp difference between block X and block X-1
pub async fn calc_relay_tps(
	api: &Api,
	n: usize,
	genesis: bool,
	prometheus_metrics: Option<StpsMetrics>,
) -> Result<(), Error> {
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
				.at(block_hash_genesis)
				.fetch(&storage_timestamp_storage_addr)
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
				.at(block_hash_x_minus_one)
				.fetch(&storage_timestamp_storage_addr)
				.await?
				.unwrap();
			(block_number_x, block_timestamp_x_minus_one)
		},
	};

	let mut total_count = 0;
	let mut tps_vec = Vec::new();

	loop {
		let block_hash_x = api
			.rpc()
			.block_hash(Some(block_number_x.into()))
			.await?
			.expect("block number exists, therefore hash exists; qed");
		let block_timestamp_x = api
			.storage()
			.at(block_hash_x)
			.fetch(&storage_timestamp_storage_addr)
			.await?
			.unwrap();
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

		// send metrics to prometheus, if enabled
		if let Some(metrics) = &prometheus_metrics {
			metrics.set(tps_count, time_diff, block_number_x);
		}

		block_number_x += 1;
		if total_count >= n {
			let avg_tps: f32 = tps_vec.iter().sum::<f32>() / tps_vec.len() as f32;
			info!("Average TPS: {}", avg_tps);
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
	let genesis = args.genesis;
	let default_parablock_time = args.default_parablock_time;
	let expected_transactions = args.num;
	let prometheus_metrics = match args.prometheus {
		true => Some(run_prometheus_endpoint(&args.prometheus_url, &args.prometheus_port).await?),
		false => None,
	};

	match para_finality {
		true => {
			if genesis == true {
				panic!("Cannot start TPS in parachain mode with --genesis flag!");
			}

			let validator_url = match args.validator_url {
				Some(s) => s,
				None => panic!(
					"Must set --collator-url and --validator-url when enabling --para-finality!"
				),
			};
			let collator_url = match args.collator_url {
				Some(s) => s,
				None => panic!(
					"Must set --collator-url and --validator-url when enabling --para-finality!"
				),
			};
			let para_id = match args.para_id {
				Some(id) => id,
				None => panic!("Must set --para-id to specify which parachain to track when enabling --para-finality!")
			};

			info!("Starting TPS in parachain mode");
			let relay_api = connect(&validator_url).await?;
			let para_api = connect(&collator_url).await?;

			// Create the channels
			let (parablock_hash_sender, parablock_hash_receiver) = channel::<H256>(100);
			let (signal_sender, signal_receiver) = oneshot_channel::<bool>();

			info!("Spawning async task to subscribe to parablocks!");
			tokio::spawn(async move {
				match subscribe(relay_api, para_id, signal_receiver, parablock_hash_sender).await {
					Ok(_) => (),
					Err(err) => panic!("Error! {:?}", err)
				}
			});
			calc_para_tps(para_api, parablock_hash_receiver, signal_sender, default_parablock_time, expected_transactions, prometheus_metrics).await?;
		},
		false => {
			let node_url = match args.node_url {
				Some(s) => s,
				None => panic!("Must pass --node-url when setting --para-finality to 'false'"),
			};
			info!("Starting TPS in relaychain mode");
			let n_tx_truncated = (args.num / args.total_senders) * args.total_senders;
			let api = connect(&node_url).await?;
			calc_relay_tps(&api, n_tx_truncated, args.genesis, prometheus_metrics).await?;
		},
	}

	Ok(())
}
