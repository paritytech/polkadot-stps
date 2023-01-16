use clap::Parser;
use log::*;
use tokio::time::{sleep, Duration};
use utils::{connect, runtime, Api, Error};

/// util program to count TPS
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
	/// Node URL
	#[arg(long)]
	node_url: String,

	/// Total number of senders
	#[arg(long)]
	total_senders: usize,

	/// Total number of expected transactions
	#[arg(short)]
	n: usize,
}

pub async fn calc_tps(api: &Api, n: usize) -> Result<(), Error> {
	let storage_timestamp_storage_addr = runtime::storage().timestamp().now();

	let block_1_hash = api.rpc().block_hash(Some(1u32.into())).await?;

	let mut last_block_timestamp = api
		.storage()
		.fetch(&storage_timestamp_storage_addr, block_1_hash)
		.await?
		.unwrap();

	let mut block_n: u32 = 2;
	let mut total_count = 0;
	let mut tps_vec = Vec::new();

	loop {
		let mut block_hash = api.rpc().block_hash(Some(block_n.into())).await?;
		while block_hash.is_none() {
			info!("waiting for finalization of block {}", block_n);
			sleep(Duration::from_secs(6)).await;

			block_hash = api.rpc().block_hash(Some(block_n.into())).await?;
		}

		let block_timestamp =
			api.storage().fetch(&storage_timestamp_storage_addr, block_hash).await?.unwrap();
		let time_diff = block_timestamp - last_block_timestamp;
		last_block_timestamp = block_timestamp;

		let mut tps_count = 0;
		let events = api.events().at(block_hash).await?;
		for event in events.iter().flatten() {
			if event.pallet_name() == "Balances" && event.variant_name() == "Transfer" {
				total_count += 1;
				tps_count += 1;
			}
		}

		if tps_count > 0 {
			let tps = tps_count as f32 / (time_diff as f32 / 1000.0);
			tps_vec.push(tps);
			info!("TPS on block {}: {}", block_n, tps);
		}

		block_n += 1;
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

	let n_tx_truncated = (args.n / args.total_senders) * args.total_senders;

	let api = connect(&args.node_url).await?;
	calc_tps(&api, n_tx_truncated).await?;

	Ok(())
}
