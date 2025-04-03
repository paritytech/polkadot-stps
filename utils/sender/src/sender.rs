use futures::{stream::FuturesUnordered, StreamExt};
use clap::Parser;
use codec::Decode;
use log::*;
use sender_lib::{connect, sign_balance_transfers};
use std::{collections::HashMap, error::Error};
use subxt::OnlineClient;
use sp_core::{ecdsa, Pair};
use stps_config::eth::{AccountId20, EthereumSigner, MythicalConfig};
use subxt::config::DefaultExtrinsicParamsBuilder;
use subxt::dynamic::Value;
use subxt::tx::Signer;

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

use jsonrpsee_client_transport::ws::WsTransportClientBuilder;
use jsonrpsee_core::client::Client;
use subxt::backend::legacy::LegacyBackend;
use std::sync::Arc;
use tokio::time::Duration;

async fn get_nonce(api: &OnlineClient<MythicalConfig>, account: AccountId20) -> u64 {
	let account_state_storage_addr = subxt::dynamic::storage("System", "Account", vec![subxt::dynamic::Value::from_bytes(account.0)]);
    let account_state_enc = api
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
    account_state.nonce as u64
}

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

	let sender_accounts = funder_lib::derive_accounts::<ecdsa::Pair>(n_tx_sender, SENDER_SEED.to_owned());
	let receiver_accounts = funder_lib::derive_accounts::<ecdsa::Pair>(n_tx_sender, RECEIVER_SEED.to_owned());

    // Fund senders and receivers

    info!("Funding accounts");
    
    const ED: u128 = 10_000_000_000_000_000;
    const BATCH_BY: usize = 250;

    let alith = ecdsa::Pair::from_seed(&subxt_signer::eth::dev::alith().secret_key());
    let alith_signer = EthereumSigner::from(alith.clone());
    let mut alith_nonce = get_nonce(&api, alith_signer.account_id()).await;

    let txs = (0..(sender_accounts.len() / BATCH_BY)).map(|i| {
        let mut batch_calls = sender_accounts.iter().skip(i * BATCH_BY).take(BATCH_BY).map(|acc| 
            subxt::dynamic::tx(
                "Balances",
                "transfer_keep_alive",
                vec![
                    Value::from_bytes(EthereumSigner::from(acc.clone()).account_id().0),
                    Value::u128(30 * ED),
                ],
            ).into_value()
        ).collect::<Vec<_>>();
        batch_calls.extend(receiver_accounts.iter().skip(i * BATCH_BY).take(BATCH_BY).map(|acc| 
            subxt::dynamic::tx(
                "Balances",
                "transfer_keep_alive",
                vec![
                    Value::from_bytes(EthereumSigner::from(acc.clone()).account_id().0),
                    Value::u128(ED),
                ],
            ).into_value()
        ));
        let batch = subxt::dynamic::tx(
            "Utility",
            "batch",
            vec![ Value::named_composite(vec![("calls", batch_calls.into())]) ]
        );

        let tx_params = DefaultExtrinsicParamsBuilder::new().nonce(alith_nonce).build();
        alith_nonce += 1;
        api.tx().create_partial_offline(&batch, tx_params).unwrap().sign(&alith_signer)
    }).collect::<Vec<_>>(); //<FuturesUnordered<_>>();

    let futs = txs.iter().map(|tx| tx.submit_and_watch()).collect::<FuturesUnordered<_>>();
	let submitted = futs.collect::<Vec<_>>().await.into_iter().collect::<Result<Vec<_>, _>>().expect("All the funding transactions submitted successfully");
    let res = submitted.into_iter().map(|tx| tx.wait_for_finalized()).collect::<FuturesUnordered<_>>();
    let _ = res.collect::<Vec<_>>().await.into_iter().collect::<Result<Vec<_>, _>>().expect("All the funding transactions finalized successfully");

    info!("Funding accounts done");

	let now = std::time::Instant::now();

	let futs = sender_accounts.iter().map(|a| {
		let account_id = EthereumSigner::from(a.clone()).account_id();
		let fapi = api.clone();
		// let account_state_storage_addr = subxt::dynamic::storage("System", "Account", vec![subxt::dynamic::Value::from_bytes(pubkey)]);
		async move {
            let nonce = get_nonce(&fapi, account_id).await;
			// let account_state_enc = fapi
			// 	.storage()
			// 	.at_latest()
			// 	.await
			// 	.expect("Storage API available")
			// 	.fetch(&account_state_storage_addr)
			// 	.await
			// 	.expect("Account status fetched")
			// 	.expect("Nonce is set")
			// 	.into_encoded();
			// let account_state: AccountInfo = Decode::decode(&mut &account_state_enc[..]).expect("Account state decodes successfuly");

			(account_id, nonce)
		}
	}).collect::<FuturesUnordered<_>>();
	let noncemap = futs.collect::<Vec<_>>().await.into_iter().collect::<HashMap<_, _>>();

	let elapsed = now.elapsed();
	info!("Got nonces in {:?}", elapsed);

	let now = std::time::Instant::now();
	let txs = sign_balance_transfers(api, sender_accounts.into_iter().map(|sa| (sa.clone(), noncemap[&EthereumSigner::from(sa).account_id()] as u64)).zip(receiver_accounts.into_iter()));
	let elapsed = now.elapsed();
	info!("Signed in {:?}", elapsed);

	info!("Starting sender in parallel mode");
	// let (producer, consumer) = tokio::sync::mpsc::unbounded_channel();
	// I/O Bound
	// pre::parallel_pre_conditions(&api, args.threads, n_tx_sender).await?;
	// // CPU Bound
	// match sender_lib::parallel_signing(&api, args.threads, n_tx_sender, producer) {
	// 	Ok(_) => (),
	// 	Err(e) => panic!("Error: {:?}", e),
	// }
	// // I/O Bound
	sender_lib::submit_txs(txs).await?;

	Ok(())
}
