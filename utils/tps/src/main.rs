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
	#[arg(long)]
	num: usize,

	/// Start scraping from genesis
	#[arg(short, long, default_value_t = false)]
	genesis: bool,
}

/// we loop over blocks and count the number of transfers in each block
/// we take the timestamp difference between block X and block X-1
pub async fn calc_tps(api: &Api, n: usize, genesis: bool) -> Result<(), Error> {
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

	let n_tx_truncated = (args.num / args.total_senders) * args.total_senders;

	let api = connect(&args.node_url).await?;

	calc_tps(&api, n_tx_truncated, args.genesis).await?;

	Ok(())
}
