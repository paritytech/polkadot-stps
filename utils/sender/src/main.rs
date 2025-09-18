use clap::Parser;
use codec::Decode;
use futures::TryStreamExt;
use log::*;
use std::{
	collections::VecDeque,
	error::Error,
	sync::atomic::{AtomicU64, Ordering},
	time::Instant, u128,
};

use sp_core::{sr25519::Pair as SrPair, Pair};
use subxt::{
	blocks::BlockRef, config::polkadot::PolkadotExtrinsicParamsBuilder as Params, dynamic::Value, ext::scale_value::{Primitive, ValueDef}, tx::SubmittableTransaction, OnlineClient, PolkadotConfig
};
use tokio::sync::RwLock;

use sender_lib::PairSigner;

const SENDER_SEED: &str = "//Sender";
const RECEIVER_SEED: &str = "//Receiver";
const ALICE_SEED: &str = "//Alice";

/// Amount to send in each transaction, small so that we can do many transactions before
/// running out of funds.
const SMALL_TOKEN_AMOUNT: Value = Value { value: ValueDef::Primitive(Primitive::U128(1)), context: () };

/// Amount to seed each sender with, largest possible value so that we do not run out of funds.
const BIG_TOKEN_AMOUNT: Value = Value { value: ValueDef::Primitive(Primitive::U128(u128::MAX)), context: () };


/// Util program to send transactions
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
	/// Node URL. Can be either a collator, or relaychain node based on whether you want to measure parachain TPS, or relaychain TPS.
	#[arg(long)]
	node_url: String,

	/// Total number of senders
	#[arg(long)]
	total_senders: Option<usize>,

	/// Chunk size for sending the extrinsics.
	#[arg(long, default_value_t = 50)]
	chunk_size: usize,

	/// Total number of pre-funded accounts (on funded-accounts.json).
	#[arg(long)]
	tps: usize,

	/// Send in batch mode with the batch size this large.
	#[arg(long, default_value_t = 1)]
	batch: usize,

	/// Seed the sender accounts
	#[arg(
		long,
        default_value_t = false,
        default_missing_value = "false",
        num_args = 0..=1,
        require_equals = false,
    )]
	seed: bool,
}

// FIXME: This assumes that all the chains supported by sTPS use this `AccountInfo` type. Currently,
// that holds. However, to benchmark a chain with another `AccountInfo` structure, a mechanism to
// adjust this type info should be provided.
type AccountInfo = frame_system::AccountInfo<u32, pallet_balances::AccountData<u128>>;

use jsonrpsee_client_transport::ws::WsTransportClientBuilder;
use jsonrpsee_core::client::{async_client::PingConfig, Client};
use std::sync::Arc;
use subxt::backend::legacy::LegacyBackend;

use tokio::time::Duration;

async fn get_account_nonce<C: subxt::Config>(
	api: &OnlineClient<C>,
	block: BlockRef<C::Hash>,
	account: &SrPair,
) -> u64 {
	let pubkey = account.public();
	let account_state_storage_addr = subxt::dynamic::storage(
		"System",
		"Account",
		vec![subxt::dynamic::Value::from_bytes(pubkey)],
	);

	let account_state_enc = api
		.storage()
		.at(block)
		.fetch(&account_state_storage_addr)
		.await
		.expect("Account status fetched")
		.expect("Nonce is set")
		.into_encoded();

	let account_state: AccountInfo =
		Decode::decode(&mut &account_state_enc[..]).expect("Account state decodes successfuly");
	account_state.nonce.into()
}

fn main() -> Result<(), Box<dyn Error>> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	let args = Args::parse();

	// Assume number of senders equal to TPS if not specified.
	let n_sender_tasks = if args.batch > 1 { args.tps / args.batch } else { args.tps };
	let n_tx_sender = args.total_senders.unwrap_or(args.tps);
	let worker_sleep =
		(1_000f64 * ((n_sender_tasks as f64 * args.batch as f64) / args.tps as f64)) as u64;

	log::info!("worker_sleep = {}", worker_sleep);
	log::info!("sender tasks  = {}", n_sender_tasks);
	log::info!("sender accounts  = {}", n_tx_sender);

	let sender_accounts = funder_lib::derive_accounts(n_tx_sender, SENDER_SEED.to_owned());
	let receiver_accounts = funder_lib::derive_accounts(n_tx_sender, RECEIVER_SEED.to_owned());
	let alice = <SrPair as Pair>::from_string(&ALICE_SEED, None).unwrap();
	let alice_signer = PairSigner::new(alice.clone());

	async fn create_api(node_url: String) -> OnlineClient<PolkadotConfig> {
		let node_url = url::Url::parse(&node_url).unwrap();
		let (node_sender, node_receiver) =
			WsTransportClientBuilder::default().build(node_url.clone()).await.unwrap();
		let client = Client::builder()
			.request_timeout(Duration::from_secs(10))
			.max_buffer_capacity_per_subscription(16 * 1024 * 1024)
			.enable_ws_ping(PingConfig::new().ping_interval(Duration::from_secs(10)))
			.set_tcp_no_delay(true)
			.max_concurrent_requests(1024 * 10)
			.build_with_tokio(node_sender, node_receiver);
		let backend = Arc::new(LegacyBackend::builder().build(client));
		OnlineClient::from_backend(backend).await.unwrap()
	}

	if args.seed {
		log::info!("Seeding accounts");

		tokio::runtime::Builder::new_multi_thread()
			.enable_all()
			.build()
			.unwrap()
			.block_on(async {
				let node_url = args.node_url.clone();
				let api = create_api(node_url.clone()).await;
				let mut best_block_stream =
					api.blocks().subscribe_best().await.expect("Subscribe to best block failed");
				let best_block = best_block_stream.next().await.unwrap().unwrap();
				let block_ref: BlockRef<subxt::utils::H256> =
					BlockRef::from_hash(best_block.hash());

				let mut nonce = get_account_nonce(&api, block_ref.clone(), &alice).await;

				for sender in sender_accounts.iter() {
					let payload = subxt::dynamic::tx(
						"Balances",
						"transfer_keep_alive",
						vec![
							Value::unnamed_variant("Id", [Value::from_bytes(sender.public())]),
							BIG_TOKEN_AMOUNT,
						],
					);

					let tx_params = Params::new().nonce(nonce as u64).build();

					let tx: SubmittableTransaction<_, OnlineClient<_>> = api
						.tx()
						.create_partial(&payload, &alice_signer.account_id(), tx_params)
						.await
						.unwrap()
						.sign(&alice_signer);

					let _ = match tx.submit_and_watch().await {
						Ok(watch) => {
							log::info!("Seeded account");
							nonce += 1;
							watch
						},
						Err(err) => {
							log::warn!("{:?}", err);
							continue;
						},
					};
				}
			});
	}

	while !args.seed {
		tokio::runtime::Builder::new_multi_thread()
			.enable_all()
			.build()
			.unwrap()
			.block_on(
				async {
				let node_url = args.node_url.clone();
				let api = create_api(node_url.clone()).await;

				// Subscribe to best block stream
				let mut best_block_stream  = api.blocks().subscribe_best().await.expect("Subscribe to best block failed");
				let best_block = Arc::new(RwLock::new((best_block_stream.next().await.unwrap().unwrap(), Instant::now())));

				log::info!("Current best block: {}", best_block.read().await.0.number() );

				let sender_signers = sender_accounts
					.iter()
					.cloned()
					.map(PairSigner::new)
					.collect::<Vec<_>>();

				info!("Starting senders");

				// Overall metrics that we use to throttle
				// Transactions sent since last block
				let sent = Arc::new(AtomicU64::default());
				// Number of in block transactions.
				let in_block = Arc::new(AtomicU64::default());

				let mut handles = Vec::new();
				let mut timestamp = Duration::from_micros(0);
				let mut block_time = Duration::from_micros(0);

				loop {

					sent.store(0, Ordering::SeqCst);
					in_block.store(0, Ordering::SeqCst);

					// Spawn 1 task per sender.
					for i in 0..n_sender_tasks {
						let in_block = in_block.clone();
						let sent = sent.clone();

						let sender = sender_accounts[i].clone();
						let signer: PairSigner = sender_signers[i].clone();
						let best_block = best_block.clone();
						let sent = sent.clone();
						let in_block = in_block.clone();

						let api = api.clone();
						let nrecv = if args.batch > 1 { args.batch } else { 1 };
						let receiver_accounts = receiver_accounts.clone();

						let task = async move {
							// Slowly ramp up 10ms slots.
							tokio::time::sleep(std::time::Duration::from_millis(((n_sender_tasks - i)*10) as u64)).await;

							let receivers = &receiver_accounts[i..i+nrecv];
							let mut sleep_time_ms = 0u64;
							let block_ref: BlockRef<subxt::utils::H256> = BlockRef::from_hash(best_block.read().await.0.hash());
							let mut nonce = get_account_nonce(&api, block_ref.clone(), &sender).await;

							loop {
								// Throttle if the backlog of un included txs is too high 
								if sent.load(Ordering::SeqCst) > in_block.load(Ordering::SeqCst) + 100_000 {
									// Wait 10ms and check again.
									tokio::time::sleep(std::time::Duration::from_millis(10)).await;
									// Substract above sleep from TPS delay.
									sleep_time_ms = sleep_time_ms.saturating_sub(10);
									continue
								}

								// Target a rate per worker, so we wait.
								tokio::time::sleep(std::time::Duration::from_millis(sleep_time_ms)).await;
								let now = Instant::now();
								log::debug!("Sender {} using nonce {}", i, nonce);

								let tx_payload = if args.batch > 1 {
									let calls = (0..args.batch).map(|i|
										subxt::dynamic::tx(
											"Balances",
											"transfer_keep_alive",
											vec![
												Value::unnamed_variant("Id", [Value::from_bytes(receivers[i].public())]),
												SMALL_TOKEN_AMOUNT,
											],
										).into_value()
									).collect::<Vec<_>>();

									subxt::dynamic::tx(
										"Utility",
										"batch",
										vec![ Value::named_composite(vec![("calls", calls.into())]) ]
									)
								} else {
									subxt::dynamic::tx(
										"Balances",
										"transfer_keep_alive",
										vec![
											Value::unnamed_variant("Id", [Value::from_bytes(receivers[0].public())]),
											SMALL_TOKEN_AMOUNT,
										],
									)
								};
								log::debug!("Sender {} using nonce {}", i, nonce);
								let tx_params = Params::new().nonce(nonce as u64).build();

								let tx: SubmittableTransaction::<_, OnlineClient<_>> = api
									.tx()
									.create_partial_offline(&tx_payload, tx_params)
									.expect("Failed to create partial offline transaction")
									.sign(&signer);

								match tx.submit_and_watch().await {
									Ok(_watch) => {},
									Err(err) => {
										log::error!("{:?}", err);
										let block_ref: BlockRef<subxt::utils::H256> = BlockRef::from_hash(best_block.read().await.0.hash());
										nonce = get_account_nonce(&api, block_ref, &sender).await;
										// at most 1 second
										sleep_time_ms = worker_sleep.saturating_sub(now.elapsed().as_millis() as u64);
										continue
									}
								};


								sent.fetch_add(args.batch as u64, Ordering::SeqCst);
								// Determine how much left to sleep, we need to retry in 1000ms (backoff)
								sleep_time_ms = worker_sleep.saturating_sub(now.elapsed().as_millis() as u64);
								nonce += 1;
							}
						};
						handles.push(tokio::spawn(task));
					}

					log::info!("All senders started");

					let mut tps_window = VecDeque::new();
					let loop_start = Instant::now();

					loop {
						if let Ok(Some(new_best_block)) = best_block_stream.try_next().await {
							*best_block.write().await = (new_best_block, Instant::now());
						} else {
							log::error!("Best block subscription lost, trying to reconnect ... ");
							loop {
								match api.blocks().subscribe_best().await {
									Ok(fresh_best_block_stream) => {
										best_block_stream = fresh_best_block_stream;
										log::info!("Reconnected.");
										break;
									}
									Err(e) => {
										log::error!("Reconnect failed: {:?} ", e);
										tokio::time::sleep(std::time::Duration::from_millis(500)).await;
									}
								}
							}
						}

						let best_block = &best_block.read().await.0;
						let Ok(extrinsics) = best_block.extrinsics().await else {
							// Most likely, need to reconnect to RPC.
							continue
						};

						let mut txcount = 0;

						for ex in extrinsics.iter() {
							match (ex.pallet_name().expect("pallet name"), ex.variant_name().expect("variant name")) {
								("Timestamp", "set") => {
									let new_timestamp = Duration::from_millis(codec::Compact::<u64>::decode(&mut &ex.field_bytes()[..]).expect("timestamp decodes").0);
									block_time =  new_timestamp - timestamp;
									timestamp = new_timestamp;
								},
								("Nfts", "transfer") => {
									txcount += 1;
								},
								_ => (),
							}
						}

						for ev in best_block.events().await.expect("Events are available").iter() {
							let ev = ev.expect("Event is available");
							match (ev.pallet_name(), ev.variant_name()) {
								("Balances", "Transfer") => {
									txcount += 1;
								},
								_ => (),
							}
						}

						in_block.fetch_add(txcount , Ordering::SeqCst);
						let btime = if block_time.is_zero() { 6000 } else { block_time.as_millis() };
						let tps = txcount * 1000 / btime as u64;
						tps_window.push_back(tps as usize);

						// A window of size 12
						if tps_window.len() > 12 {
							tps_window.pop_front();
							let avg_tps = tps_window.iter().sum::<usize>();
							if avg_tps < args.tps / 4 {
								log::warn!("TPS dropped below 25% of target ...");
								break;
							}
						}

						let avg_tps = tps_window.iter().sum::<usize>() / tps_window.len();

						log::info!("TPS: {} \t | Avg: {} \t | Sent/Exec: {}/{} | Best: {} | txs = {} | block time = {:?}", tps, avg_tps, sent.load(Ordering::SeqCst),  in_block.load(Ordering::SeqCst), best_block.number(), txcount, block_time);
						if loop_start.elapsed() > Duration::from_secs(60 * 5) {
							break;
						}
					}

					// Restarting
					for handle in handles.iter() {
						handle.abort();
					}
					log::info!("Restarting senders");
				}
			}
		);
	}
	Ok(())
}
