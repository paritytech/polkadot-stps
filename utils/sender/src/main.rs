use clap::Parser;
use codec::Decode;
use futures::TryStreamExt;
use log::*;
use std::{
	collections::VecDeque,
	error::Error,
	sync::atomic::{AtomicU64, Ordering},
	time::Instant,
};
// use subxt::{ext::sp_core::Pair, utils::AccountId32, OnlineClient, PolkadotConfig};

use sp_core::{sr25519::Pair as SrPair, Pair};
use subxt::{
	blocks::BlockRef, config::polkadot::PolkadotExtrinsicParamsBuilder as Params, dynamic::Value,
	tx::SubmittableTransaction, OnlineClient, PolkadotConfig,
};
use tokio::sync::RwLock;

use sender_lib::PairSigner;

const SENDER_SEED: &str = "//Sender";
const RECEIVER_SEED: &str = "//Receiver";
const ALICE_SEED: &str = "//Alice";
const BACKLOG_THRESHOLD: u64 = 100_000;
const SEED_TRANSFER_AMOUNT: u128 = 100_000_000_000_000_000_000; // 1e20
const TX_TRANSFER_AMOUNT: u128 = 1_000_000_000_000; // 1e12
const DEFAULT_BLOCK_TIME_MS: u64 = 6_000;
const RAMP_SLOT_MS: u64 = 10;
const RETRY_THROTTLE_MS: u64 = 10;
const RECONNECT_SLEEP_MS: u64 = 500;

/// CLI utility to generate transaction load against a Substrate-based node.
///
/// Spawns multiple sender workers that continuously submit balance transfers
/// (optionally in batches), monitors best blocks to compute TPS, and throttles
/// submissions based on the backlog of un-included transactions. Use `--seed`
/// to pre-fund derived sender accounts from `//Alice` before starting.
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
/// Type alias for decoding `System::Account` storage used to extract the account nonce.
type AccountInfo = frame_system::AccountInfo<u32, pallet_balances::AccountData<u128>>;

use jsonrpsee_client_transport::ws::WsTransportClientBuilder;
use jsonrpsee_core::client::{async_client::PingConfig, Client};
use std::sync::Arc;
use subxt::backend::legacy::LegacyBackend;

use tokio::time::Duration;

/// Static configuration for sender workers.
///
/// - `n_sender_tasks`: number of concurrent worker tasks
/// - `batch`: number of transfers per extrinsic (Utility::batch)
/// - `worker_sleep`: target per-worker pacing delay in milliseconds
/// - `tps`: global target TPS for monitoring/early stop
#[derive(Clone, Copy, Debug)]
struct WorkerConfig {
    n_sender_tasks: usize,
    batch: usize,
    worker_sleep: u64,
    tps: usize,
}

/// Shared state used for throttling and metrics.
///
/// - `sent`: total submitted extrinsics
/// - `in_block`: total extrinsics observed included in blocks
#[derive(Clone, Debug)]
struct SharedState {
    sent: Arc<AtomicU64>,
    in_block: Arc<AtomicU64>,
}

/// Owned inputs needed by a single worker.
///
/// `receivers` should be sized according to `batch` (or 1 if not batching).
#[derive(Clone)]
struct WorkerInputs {
    sender: SrPair,
    signer: PairSigner,
    receivers: Vec<SrPair>,
}

/// Fetch the current nonce for `account` at the given `block`.
///
/// Returns the decoded `frame_system::AccountInfo` nonce as `u64`.
async fn fetch_account_nonce_at_block<C: subxt::Config>(
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

/// Connect to the node and return an `OnlineClient<PolkadotConfig>` using the legacy backend.
async fn connect_online_client(node_url: String) -> OnlineClient<PolkadotConfig> {
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

/// Transfer funds from `//Alice` to the provided `sender_accounts` so that they can submit txs.
///
/// Uses `Balances::transfer_keep_alive` and increments the nonce for each transfer.
fn seed_sender_accounts(node_url: &str, alice: &SrPair, sender_accounts: &[SrPair]) {
	log::info!("Seeding accounts");
	tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.unwrap()
		.block_on(async {
			let api = connect_online_client(node_url.to_owned()).await;
			let mut best_block_stream =
				api.blocks().subscribe_best().await.expect("Subscribe to best block failed");
			let best_block = best_block_stream.next().await.unwrap().unwrap();
			let block_ref: BlockRef<subxt::utils::H256> = BlockRef::from_hash(best_block.hash());

			let mut nonce = fetch_account_nonce_at_block(&api, block_ref.clone(), alice).await;

			for sender in sender_accounts.iter() {
				let payload = subxt::dynamic::tx(
					"Balances",
					"transfer_keep_alive",
					vec![
						Value::unnamed_variant("Id", [Value::from_bytes(sender.public())]),
						Value::u128(SEED_TRANSFER_AMOUNT),
					],
				);

				let tx_params = Params::new().nonce(nonce as u64).build();

				let alice_signer = PairSigner::new(alice.clone());

                let tx: SubmittableTransaction<_, OnlineClient<_>> = api
                    .tx()
                    .create_partial(&payload, alice_signer.account_id(), tx_params)
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

/// Continuously submit transactions (blocking wrapper).
///
/// Runs a multi-threaded runtime that continuously spawns sender workers which
/// submit transactions and retry on failures when possible.
fn submit_transactions_continuously_blocking(
    sender_accounts: &[SrPair],
    receiver_accounts: Vec<SrPair>,
    node_url: &str,
    n_sender_tasks: usize,
    batch: usize,
    worker_sleep: u64,
    tps: usize,
) {
	tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.unwrap()
		.block_on(async {
			submit_transactions_continuously(
				sender_accounts,
				receiver_accounts.clone(),
				node_url,
				n_sender_tasks,
				batch,
				worker_sleep,
				tps,
			)
			.await
		});
}

/// Run sender workers and monitor the best block stream for metrics and health.
///
/// Spawns one task per sender, logs TPS metrics, and restarts if the stream drops.
async fn run_sender_workers(
    api: OnlineClient<PolkadotConfig>,
    sender_accounts: &[SrPair],
    sender_signers: &[PairSigner],
    receiver_accounts: Vec<SrPair>,
    cfg: WorkerConfig,
    shared: &SharedState,
    handles: &mut Vec<tokio::task::JoinHandle<()>>,
) {
	// Subscribe to best block stream and establish initial best block reference
	let mut best_block_stream =
		api.blocks().subscribe_best().await.expect("Subscribe to best block failed");
	let best_block =
		Arc::new(RwLock::new((best_block_stream.next().await.unwrap().unwrap(), Instant::now())));
	log::info!("Current best block: {}", best_block.read().await.0.number());

	// Local timing state for TPS calculation
	let mut timestamp = Duration::from_micros(0);
	let mut block_time = Duration::from_micros(0);

    shared.sent.store(0, Ordering::SeqCst);
    shared.in_block.store(0, Ordering::SeqCst);

	// Spawn 1 task per sender.
    for i in 0..cfg.n_sender_tasks {
        // Helper closure to fetch the current best block hash without exposing its concrete type.
        let best_block_c = best_block.clone();
        let get_best_hash = move || {
            let best_block_c = best_block_c.clone();
            Box::pin(async move { best_block_c.read().await.0.hash() })
                as std::pin::Pin<Box<dyn std::future::Future<Output = subxt::utils::H256> + Send>>
        };

        let nrecv = if cfg.batch > 1 { cfg.batch } else { 1 };
        let inputs = WorkerInputs {
            sender: sender_accounts[i].clone(),
            signer: sender_signers[i].clone(),
            receivers: receiver_accounts[i..i + nrecv].to_vec(),
        };

        let handle = spawn_sender_worker(i, api.clone(), inputs, cfg, shared.clone(), get_best_hash);
        handles.push(handle);
    }
    log::info!("All senders started");

	let mut tps_window = VecDeque::new();
	let loop_start = Instant::now();
	loop {
		// Update best block subscription, reconnecting if necessary
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
					},
					Err(e) => {
						log::error!("Reconnect failed: {:?} ", e);
						tokio::time::sleep(std::time::Duration::from_millis(RECONNECT_SLEEP_MS)).await;
					},
				}
			}
		}
		let best_block_r = &best_block.read().await.0;
		// Process the current best block, update metrics, and decide whether to stop early.
        if evaluate_block_metrics_and_maybe_stop(
            best_block_r,
            &mut timestamp,
            &mut block_time,
            &mut tps_window,
            &shared.sent,
            &shared.in_block,
            cfg.tps,
        )
        .await
        {
            break;
        }
		if loop_start.elapsed() > Duration::from_secs(60 * 5) {
			break;
		}
	}
}

/// References required by a single sender tick.
struct TickRefs<'a, F>
where
    F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = subxt::utils::H256> + Send>>
        + Send
        + Sync
        + 'static,
{
    api: &'a OnlineClient<PolkadotConfig>,
    signer: &'a PairSigner,
    sender: &'a SrPair,
    receivers: &'a [SrPair],
    get_best_hash: &'a F,
}

/// Perform a single sender tick: throttle, build payload, sign, submit, update metrics.
async fn perform_sender_tick<F>(
    i: usize,
    nonce: &mut u64,
    sleep_time_ms: &mut u64,
    refs: &TickRefs<'_, F>,
    cfg: &WorkerConfig,
    shared: &SharedState,
) where
    F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = subxt::utils::H256> + Send>>
        + Send
        + Sync
        + 'static,
{
    // Throttle if the backlog of un-included txs is too high
    if shared.sent.load(Ordering::SeqCst) > shared.in_block.load(Ordering::SeqCst) + BACKLOG_THRESHOLD {
        // Wait 10ms and check again.
        tokio::time::sleep(std::time::Duration::from_millis(RETRY_THROTTLE_MS)).await;
        // Subtract above sleep from TPS delay.
        *sleep_time_ms = sleep_time_ms.saturating_sub(RETRY_THROTTLE_MS);
        return;
    }

    // Target a rate per worker, so we wait.
    tokio::time::sleep(std::time::Duration::from_millis(*sleep_time_ms)).await;
    let now = Instant::now();
    log::debug!("Sender {} using nonce {}", i, *nonce);

    let tx_payload = if cfg.batch > 1 {
        let calls = (0..cfg.batch)
            .map(|i| {
                subxt::dynamic::tx(
                    "Balances",
                    "transfer_keep_alive",
                    vec![
                        Value::unnamed_variant(
                            "Id",
                            [Value::from_bytes(refs.receivers[i].public())],
                        ),
                        Value::u128(TX_TRANSFER_AMOUNT),
                    ],
                )
                .into_value()
            })
            .collect::<Vec<_>>();
        subxt::dynamic::tx(
            "Utility",
            "batch",
            vec![Value::named_composite(vec![("calls", calls.into())])],
        )
    } else {
        subxt::dynamic::tx(
            "Balances",
            "transfer_keep_alive",
            vec![
                Value::unnamed_variant("Id", [Value::from_bytes(refs.receivers[0].public())]),
                Value::u128(TX_TRANSFER_AMOUNT),
            ],
        )
    };

    let tx_params = Params::new().nonce(*nonce).build();
    let tx: SubmittableTransaction<_, OnlineClient<_>> = refs
        .api
        .tx()
        .create_partial_offline(&tx_payload, tx_params)
        .expect("Failed to create partial offline transaction")
        .sign(refs.signer);

    match tx.submit_and_watch().await {
        Ok(_watch) => {
            // no-op; success path continues below
        }
        Err(err) => {
            log::error!("{:?}", err);
            let block_ref: BlockRef<subxt::utils::H256> =
                BlockRef::from_hash((refs.get_best_hash)().await);
            *nonce = fetch_account_nonce_at_block(refs.api, block_ref, refs.sender).await;
            // at most 1 second
            *sleep_time_ms = cfg.worker_sleep.saturating_sub(now.elapsed().as_millis() as u64);
            return;
        }
    };

    shared.sent.fetch_add(cfg.batch as u64, Ordering::SeqCst);
    // Determine how much left to sleep, we need to retry in 1000ms (backoff)
    *sleep_time_ms = cfg.worker_sleep.saturating_sub(now.elapsed().as_millis() as u64);
    *nonce += 1;
}

/// Spawn a single sender worker task which submits transactions in a loop.
fn spawn_sender_worker<F>(
    i: usize,
    api: OnlineClient<PolkadotConfig>,
    inputs: WorkerInputs,
    cfg: WorkerConfig,
    shared: SharedState,
    get_best_hash: F,
) -> tokio::task::JoinHandle<()>
where
    F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = subxt::utils::H256> + Send>>
        + Send
        + Sync
        + 'static,
{
    tokio::spawn(async move {
        // Slowly ramp up 10ms slots.
        tokio::time::sleep(std::time::Duration::from_millis(((cfg.n_sender_tasks - i) as u64) * RAMP_SLOT_MS))
            .await;
        let mut sleep_time_ms = 0u64;
        let block_ref: BlockRef<subxt::utils::H256> = BlockRef::from_hash(get_best_hash().await);
        let mut nonce = fetch_account_nonce_at_block(&api, block_ref.clone(), &inputs.sender).await;
        loop {
            let refs = TickRefs {
                api: &api,
                signer: &inputs.signer,
                sender: &inputs.sender,
                receivers: &inputs.receivers,
                get_best_hash: &get_best_hash,
            };
            perform_sender_tick(i, &mut nonce, &mut sleep_time_ms, &refs, &cfg, &shared).await;
        }
    })
}

/// Parse the current best block, update TPS-related metrics, and log a summary.
///
/// Returns `true` when TPS drops below a threshold, signaling an early stop.
async fn evaluate_block_metrics_and_maybe_stop(
	best_block_r: &subxt::blocks::Block<PolkadotConfig, OnlineClient<PolkadotConfig>>,
	timestamp: &mut Duration,
	block_time: &mut Duration,
	tps_window: &mut VecDeque<usize>,
	sent: &Arc<AtomicU64>,
	in_block: &Arc<AtomicU64>,
	tps: usize,
) -> bool {
	let Ok(extrinsics) = best_block_r.extrinsics().await else {
		// Most likely, need to reconnect to RPC.
		return false;
	};
	let mut txcount: u64 = 0;
	for ex in extrinsics.iter() {
		match (ex.pallet_name().expect("pallet name"), ex.variant_name().expect("variant name")) {
			("Timestamp", "set") => {
				let new_timestamp = Duration::from_millis(
					codec::Compact::<u64>::decode(&mut &ex.field_bytes()[..])
						.expect("timestamp decodes")
						.0,
				);
				*block_time = new_timestamp.saturating_sub(*timestamp);
				*timestamp = new_timestamp;
			},
			("Nfts", "transfer") => {
				txcount += 1;
			},
			_ => (),
		}
	}
    for ev in best_block_r.events().await.expect("Events are available").iter() {
        let ev = ev.expect("Event is available");
        if let ("Balances", "Transfer") = (ev.pallet_name(), ev.variant_name()) {
            txcount += 1;
        }
    }
	in_block.fetch_add(txcount, Ordering::SeqCst);
    let btime_ms = if block_time.is_zero() { DEFAULT_BLOCK_TIME_MS } else { block_time.as_millis() as u64 };
	let tps_ = txcount.saturating_mul(1000) / btime_ms.max(1);
	tps_window.push_back(tps_ as usize);
	// Keep window size to 12
	if tps_window.len() > 12 {
		tps_window.pop_front();
		let avg_tps = tps_window.iter().sum::<usize>();
		if avg_tps < tps / 4 {
			log::warn!("TPS dropped below 25% of target ...");
			return true;
		}
	}
	let avg_tps = tps_window.iter().sum::<usize>() / tps_window.len();
	log::info!(
		"TPS: {} \t | Avg: {} \t | Sent/Exec: {}/{} | Best: {} | txs = {} | block time = {:?}",
		tps,
		avg_tps,
		sent.load(Ordering::SeqCst),
		in_block.load(Ordering::SeqCst),
		best_block_r.number(),
		txcount,
		*block_time
	);
	false
}

/// Continuously submit transactions and retry on failure.
///
/// Connects to the node and continuously (re)launches sender workers which
/// submit transactions; on errors they back off, refresh nonce if needed, and retry.
async fn submit_transactions_continuously(
    sender_accounts: &[SrPair],
    receiver_accounts: Vec<SrPair>,
    node_url: &str,
    n_sender_tasks: usize,
    batch: usize,
    worker_sleep: u64,
    tps: usize,
) {
    let node_url = node_url.to_owned();
	let api = connect_online_client(node_url.clone()).await;
	let sender_signers = sender_accounts.iter().cloned().map(PairSigner::new).collect::<Vec<_>>();
    info!("Starting senders");
    // Overall metrics that we use to throttle
    let shared = SharedState { sent: Arc::new(AtomicU64::default()), in_block: Arc::new(AtomicU64::default()) };
	let mut handles = Vec::new();
	loop {
        run_sender_workers(
            api.clone(),
            sender_accounts,
            &sender_signers,
            receiver_accounts.clone(),
            WorkerConfig { n_sender_tasks, batch, worker_sleep, tps },
            &shared,
            &mut handles,
        )
        .await;

		// Restarting
		for handle in handles.iter() {
			handle.abort();
		}
		log::info!("Restarting senders");
	}
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
    let alice = <SrPair as Pair>::from_string(ALICE_SEED, None).unwrap();

	if args.seed {
		seed_sender_accounts(&args.node_url, &alice, &sender_accounts);
	}

	loop {
		submit_transactions_continuously_blocking(
			&sender_accounts,
			receiver_accounts.clone(),
			&args.node_url,
			n_sender_tasks,
			args.batch,
			worker_sleep,
			args.tps,
		);
	}
}
