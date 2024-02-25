use clap::Parser;
use log::*;
use utils::{connect, Error};

mod pre;

/// Util program to send transactions
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
	/// Node URL. Can be either a collator, or relaychain node based on whether you want to measure parachain TPS, or relaychain TPS.
	#[arg(long)]
	node_url: String,

	/// Set to the number of desired threads (default: 1). If set > 1 the program will spawn multiple threads to send transactions in parallel.
	#[arg(long, default_value_t = 1)]
	threads: usize,

	/// The sender index. Useful if you set threads to =< 1 and run multiple sender instances (as in the zombienet tests).
	#[arg(long)]
	sender_index: Option<usize>,

	/// Total number of senders
	#[arg(long)]
	total_senders: Option<usize>,

	/// Chunk size for sending the extrinsics.
	#[arg(long, default_value_t = 50)]
	chunk_size: usize,

	/// Total number of pre-funded accounts (on funded-accounts.json).
	#[arg(long)]
	num: usize,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	let args = Args::parse();
	let node_url = args.node_url;
	let threads = args.threads;
	let chunk_size = args.chunk_size;

	// This index is optional and only set when single-threaded mode is used.
	// If it is not set, we default to 0.
	let sender_index = match args.sender_index {
		Some(i) => i,
		None => 0,
	};

	// In case the optional total_senders argument is not passed for single-threaded mode,
	// we must make sure that we split the work evenly between threads for multi-threaded mode.
	let n_tx_sender = match args.total_senders {
		Some(tot_s) => args.num / tot_s,
		None => args.num / threads,
	};

	// Create the client here, so that we can use it in the various functions.
	let api = connect(&node_url).await?;

	match args.threads {
		n if n > 1 => {
			info!("Starting sender in parallel mode");
			let (producer, mut consumer) = tokio::sync::mpsc::unbounded_channel();
			// I/O Bound
			pre::parallel_pre_conditions(&api, &threads, &n_tx_sender).await?;
			// CPU Bound
			match sender_lib::parallel_signing(&api, &threads, n_tx_sender, producer) {
				Ok(_) => (),
				Err(e) => panic!("Error: {:?}", e),
			}
			// I/O Bound
			sender_lib::submit_txs(&mut consumer, chunk_size, threads).await?;
		},
		// Single-threaded mode
		n if n == 1 => {
			debug!("Starting sender in single-threaded mode");
			match args.sender_index {
				Some(i) => {
					pre::pre_conditions(&api, &i, &n_tx_sender).await?;
					sender_lib::send_funds(&api, sender_index, chunk_size, n_tx_sender).await?;
				},
				None => panic!("Must set sender index when running in single-threaded mode"),
			}
		},
		// All other non-sensical cases
		_ => panic!("Number of threads must be 1, or greater!"),
	}
	Ok(())
}
