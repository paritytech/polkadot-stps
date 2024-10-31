use futures::{stream::FuturesUnordered, StreamExt};
use clap::Parser;
use codec::Decode;
use log::*;
use sender_lib::{connect, sign_balance_transfers};
use std::{collections::HashMap, error::Error};
// use subxt::{ext::sp_core::Pair, utils::AccountId32, OnlineClient, PolkadotConfig};

use subxt::{
	config::polkadot::PolkadotExtrinsicParamsBuilder as Params,
	dynamic::Value,
	ext::sp_core::{sr25519::Pair as SrPair, Pair},
	tx::{PairSigner, SubmittableExtrinsic},
	OnlineClient, PolkadotConfig,
};
use tokio::sync::{Mutex, Semaphore};

const SENDER_SEED: &str = "//Sender";
const RECEIVER_SEED: &str = "//Receiver";

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

// FIXME: This assumes that all the chains supported by sTPS use this `AccountInfo` type. Currently,
// that holds. However, to benchmark a chain with another `AccountInfo` structure, a mechanism to
// adjust this type info should be provided.
type AccountInfo = frame_system::AccountInfo<u32, pallet_balances::AccountData<u128>>;

#[derive(Debug)]
enum AccountError {
	Subxt(subxt::Error),
	Codec,
}

impl std::fmt::Display for AccountError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			AccountError::Subxt(e) => write!(f, "Subxt error: {}", e.to_string()),
			AccountError::Codec => write!(f, "SCALE codec error"),
		}
	}
}

impl Error for AccountError {}

/// Check account nonce and free balance
// async fn check_account(
// 	api: OnlineClient<PolkadotConfig>,
// 	account: AccountId32,
// 	ext_deposit: u128,
// ) -> Result<(), AccountError> {
// 	let account_state_storage_addr = subxt::dynamic::storage("System", "Account", vec![account]);
// 	let finalized_head_hash = api
// 		.backend()
// 		.latest_finalized_block_ref()
// 		.await
// 		.map_err(AccountError::Subxt)?
// 		.hash();
// 	let account_state_encoded = api
// 		.storage()
// 		.at(finalized_head_hash)
// 		.fetch(&account_state_storage_addr)
// 		.await
// 		.map_err(AccountError::Subxt)?
// 		.expect("Existential deposit is set")
// 		.into_encoded();
// 	let account_state: AccountInfo =
// 		Decode::decode(&mut &account_state_encoded[..]).map_err(|_| AccountError::Codec)?;

// 	if account_state.nonce != 0 {
// 		panic!("Account has non-zero nonce");
// 	}

// 	// Reserve 10% for fees
// 	if (account_state.data.free as f64) < ext_deposit as f64 * 1.1 {
// 		panic!("Account has insufficient funds");
// 	}
// 	Ok(())
// }

use jsonrpsee_client_transport::ws::WsTransportClientBuilder;
use jsonrpsee_core::client::Client;
use subxt::backend::legacy::LegacyBackend;
use std::sync::Arc;
use tokio::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	let args = Args::parse();

	if args.threads < 1 {
		panic!("Number of threads must be 1, or greater!")
	}

	// In case the optional total_senders argument is not passed for single-threaded mode,
	// we must make sure that we split the work evenly between threads for multi-threaded mode.
	let n_tx_sender = match args.total_senders {
		Some(tot_s) => args.num / tot_s,
		None => args.num / args.threads,
	};

	// Create the client here, so that we can use it in the various functions.
	// let api = connect(&args.node_url).await?;

	let node_url = url::Url::parse(&args.node_url)?;
	let (node_sender, node_receiver) = WsTransportClientBuilder::default().build(node_url).await?;
	let client = Client::builder()
		.request_timeout(Duration::from_secs(3600))
		.max_buffer_capacity_per_subscription(4096 * 1024)
		.max_concurrent_requests(2 * 1024 * 1024)
		.build_with_tokio(node_sender, node_receiver);
	let backend = LegacyBackend::builder().build(client);
	let api = OnlineClient::from_backend(Arc::new(backend)).await?;

	let sender_accounts = funder_lib::derive_accounts(n_tx_sender, SENDER_SEED.to_owned());
	let receiver_accounts = funder_lib::derive_accounts(n_tx_sender, RECEIVER_SEED.to_owned());

	let tx_templates = receiver_accounts.iter().map(|a| 
		subxt::dynamic::tx(
			"Balances",
			"transfer_keep_alive",
			vec![
				Value::unnamed_variant("Id", [Value::from_bytes(a.public())]),
				Value::u128(1u32.into()),
			],
		)
	).collect::<Vec<_>>();

	let sender_signers = sender_accounts.iter().cloned().map(PairSigner::<PolkadotConfig, SrPair>::new).collect::<Vec<_>>();

	let now = std::time::Instant::now();

	let futs = sender_accounts.iter().map(|a| {
		let pubkey = a.public();
		let fapi = api.clone();
		let account_state_storage_addr = subxt::dynamic::storage("System", "Account", vec![subxt::dynamic::Value::from_bytes(pubkey)]);
		async move {
			let account_state_enc = fapi
				.storage()
				.at_latest()
				.await
				.expect("Storage API available")
				.fetch(&account_state_storage_addr)
				.await
				.expect("Account status fetched")
				.expect("Nonce is set")
				.into_encoded();

			let account_state: AccountInfo = Decode::decode(&mut &account_state_enc[..]).expect("Account state decodes successfuly");
			(pubkey, account_state.nonce)
		}
	}).collect::<FuturesUnordered<_>>();

	let mut noncemap = 
		// Arc::new(Mutex::new(
			futs.collect::<Vec<_>>().await.into_iter().collect::<HashMap<_, _>>()
		// ))
	;
	info!("Got nonces in {:?}", now.elapsed());

	let sema = Arc::new(Semaphore::new(n_tx_sender));

	info!("Starting sender");
	
	let mut txi = (0..n_tx_sender).cycle();

	loop {
		let i = txi.next().unwrap();
		let permit = sema.clone().acquire_owned().await.unwrap();
		// let noncemap = noncemap.clone();
		let fapi = api.clone();
		let sender = &sender_accounts[i];
		// let receiver = &receiver_accounts[i];
		let signer = sender_signers[i].clone();
		let tx_payload = tx_templates[i].clone();
		let nonceref = noncemap.get_mut(&sender.public()).unwrap();
		let nonce = *nonceref;
		*nonceref = nonce + 1;
		let tx_params = Params::new().nonce(nonce as u64).build();
		let task = async move {
			let tx = fapi.tx().create_signed_offline(&tx_payload, &signer, tx_params).unwrap();
			let mut watch = tx.submit_and_watch().await.unwrap();
			while let Some(a) = watch.next().await {
				match a {
					Ok(st) => match st {
						subxt::tx::TxStatus::Validated => { log::trace!("VALIDATED") },
						subxt::tx::TxStatus::Broadcasted { num_peers } =>
							log::trace!("BROADCASTED TO {num_peers}"),
						subxt::tx::TxStatus::NoLongerInBestBlock => log::warn!("NO LONGER IN BEST BLOCK"),
						subxt::tx::TxStatus::InBestBlock(_) => { log::trace!("IN BEST BLOCK"); break; },
						subxt::tx::TxStatus::InFinalizedBlock(_) => log::trace!("IN FINALIZED BLOCK"),
						subxt::tx::TxStatus::Error { message } => log::warn!("ERROR: {message}"),
						subxt::tx::TxStatus::Invalid { message } => log::warn!("INVALID: {message}"),
						subxt::tx::TxStatus::Dropped { message } => log::warn!("DROPPED: {message}"),
					},
					Err(e) => {
						warn!("Error status {:?}", e);
					},
				}
			}
			drop(permit);
		};
		tokio::spawn(task);
	}

	// let now = std::time::Instant::now();
	// let txs = sign_balance_transfers(api, sender_accounts.into_iter().map(|sa| (sa.clone(), noncemap[&sa.public()] as u64)).zip(receiver_accounts.into_iter()))?;
	// let elapsed = now.elapsed();
	// info!("Signed in {:?}", elapsed);

	// let (producer, consumer) = tokio::sync::mpsc::unbounded_channel();
	// I/O Bound
	// pre::parallel_pre_conditions(&api, args.threads, n_tx_sender).await?;
	// // CPU Bound
	// match sender_lib::parallel_signing(&api, args.threads, n_tx_sender, producer) {
	// 	Ok(_) => (),
	// 	Err(e) => panic!("Error: {:?}", e),
	// }
	// // I/O Bound
	// sender_lib::submit_txs(txs).await?;

	Ok(())
}
