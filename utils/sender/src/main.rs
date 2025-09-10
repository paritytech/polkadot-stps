use clap::Parser;
use codec::Decode;
use futures::TryStreamExt;
use jsonrpsee_client_transport::ws::WsTransportClientBuilder;
use jsonrpsee_core::client::{async_client::PingConfig, Client};
use sender_lib::PairSigner;
use sp_core::{sr25519::Pair as SrPair, Pair};
use std::{
	collections::VecDeque,
	error::Error,
	sync::{
		atomic::{AtomicU64, Ordering},
		Arc,
	},
	time::Instant,
};
use subxt::{
	backend::legacy::LegacyBackend, blocks::BlockRef,
	config::polkadot::PolkadotExtrinsicParamsBuilder as Params, dynamic::Value,
	tx::SubmittableTransaction, OnlineClient, PolkadotConfig,
};
use tokio::{sync::RwLock, time::Duration};

const PALLET_NAME_NFTS: &str = "Nfts";
const PALLET_NAME_TIMESTAMP: &str = "Timestamp";
const PALLET_NAME_BALANCES: &str = "Balances";
const PALLET_NAME_UTILITY: &str = "Utility";
const PALLET_NAME_SYSTEM: &str = "System";
const EXTRINSIC_VARIANT_NAME_SET: &str = "set";
const EXTRINSIC_VARIANT_NAME_TRANSFER: &str = "transfer";
const CALL_NAME_TRANSFER_KEEP_ALIVE: &str = "transfer_keep_alive";
const CALL_NAME_BATCH: &str = "batch";
const ENTRY_NAME_ACCOUNT: &str = "Account";
const EVENT_VARIANT_NAME_TRANSFER: &str = "Transfer";
const SENDER_SEED: &str = "//Sender";
const RECEIVER_SEED: &str = "//Receiver";
const ALICE_SEED: &str = "//Alice";
const BACKLOG_THRESHOLD: u64 = 100_000;
const SEED_TRANSFER_AMOUNT: u128 = 100_000_000_000_000_000_000;
const TX_TRANSFER_AMOUNT: u128 = 1_000_000_000_000;
const DEFAULT_BLOCK_TIME_MS: u64 = 6_000;
const RAMP_SLOT_MS: u64 = 10;
const RETRY_THROTTLE_MS: u64 = 10;
const RECONNECT_SLEEP_MS: u64 = 500;

type BestBlockRef =
	Arc<RwLock<(subxt::blocks::Block<PolkadotConfig, OnlineClient<PolkadotConfig>>, Instant)>>;

#[derive(Clone, Default)]
struct WorkerState {
	nonce: u64,
	sleep_time_ms: u64,
}

/// Type alias for decoding `System::Account` storage used to extract the account nonce.
type AccountInfo = frame_system::AccountInfo<u32, pallet_balances::AccountData<u128>>;

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

struct SenderApp {
	should_seed: bool,
	node_url: String,
	cfg: WorkerConfig,
	shared: SharedState,
	sender_accounts: Arc<Vec<SrPair>>,
	receiver_accounts: Arc<Vec<SrPair>>,
	sender_signers: Arc<Vec<PairSigner>>,
}

impl From<Args> for SenderApp {
	fn from(args: Args) -> Self {
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

		// Build app config
		let cfg = WorkerConfig { n_sender_tasks, batch: args.batch, worker_sleep, tps: args.tps };

		Self::new(args.seed, args.node_url, sender_accounts, receiver_accounts, cfg)
	}
}
impl SenderApp {
	fn new(
		should_seed: bool,
		node_url: String,
		sender_accounts: Vec<SrPair>,
		receiver_accounts: Vec<SrPair>,
		cfg: WorkerConfig,
	) -> Self {
		let sender_signers =
			sender_accounts.iter().cloned().map(PairSigner::new).collect::<Vec<_>>();
		let shared = SharedState {
			sent: Arc::new(AtomicU64::default()),
			in_block: Arc::new(AtomicU64::default()),
		};
		Self {
			should_seed,
			node_url,
			cfg,
			shared,
			sender_accounts: Arc::new(sender_accounts),
			receiver_accounts: Arc::new(receiver_accounts),
			sender_signers: Arc::new(sender_signers),
		}
	}

	fn run(&self) -> Result<(), Box<dyn Error>> {
		tokio::runtime::Builder::new_multi_thread()
			.enable_all()
			.build()
			.unwrap()
			.block_on(async move {
				let api = connect_online_client(self.node_url.to_owned()).await;
				let alice = <SrPair as Pair>::from_string(ALICE_SEED, None).unwrap();

				if self.should_seed {
					self.seed_senders(&api, &alice).await;
				}
				self.do_run(&api).await;
			});
		Ok(())
	}

	async fn do_run(&self, api: &OnlineClient<PolkadotConfig>) {
		loop {
			let mut handles = Vec::new();
			self.run_workers(api, &mut handles).await;
			for handle in handles.iter() {
				handle.abort();
			}
			log::info!("Restarting senders");
		}
	}

	/// Transfer funds from a seeding account to all sender accounts owned by this app.
	async fn seed_senders(&self, api: &OnlineClient<PolkadotConfig>, seeding_account: &SrPair) {
		log::info!("Seeding accounts");
		let mut best_block_stream =
			api.blocks().subscribe_best().await.expect("Subscribe to best block failed");
		let best_block = best_block_stream.next().await.unwrap().unwrap();
		let block_ref: BlockRef<subxt::utils::H256> = BlockRef::from_hash(best_block.hash());

		let mut nonce = fetch_account_nonce_at_block(api, block_ref.clone(), seeding_account).await;

		for sender in self.sender_accounts.iter() {
			let payload = subxt::dynamic::tx(
				PALLET_NAME_BALANCES,
				CALL_NAME_TRANSFER_KEEP_ALIVE,
				vec![
					Value::unnamed_variant("Id", [Value::from_bytes(sender.public())]),
					Value::u128(SEED_TRANSFER_AMOUNT),
				],
			);

			let tx_params = Params::new().nonce(nonce).build();
			let signer = PairSigner::new(seeding_account.clone());
			let tx: SubmittableTransaction<_, OnlineClient<_>> = api
				.tx()
				.create_partial(&payload, signer.account_id(), tx_params)
				.await
				.unwrap()
				.sign(&signer);

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
	}

	async fn run_workers(
		&self,
		api: &OnlineClient<PolkadotConfig>,
		handles: &mut Vec<tokio::task::JoinHandle<()>>,
	) {
		let mut best_block_stream =
			api.blocks().subscribe_best().await.expect("Subscribe to best block failed");
		let best_block: BestBlockRef = Arc::new(RwLock::new((
			best_block_stream.next().await.unwrap().unwrap(),
			Instant::now(),
		)));
		log::info!("Current best block: {}", best_block.read().await.0.number());

		let mut timestamp = Duration::from_micros(0);
		let mut block_time = Duration::from_micros(0);
		self.shared.sent.store(0, Ordering::SeqCst);
		self.shared.in_block.store(0, Ordering::SeqCst);

		for i in 0..self.cfg.n_sender_tasks {
			let handle = self.spawn_worker(api, i, best_block.clone());
			handles.push(handle);
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
						},
						Err(e) => {
							log::error!("Reconnect failed: {:?} ", e);
							tokio::time::sleep(std::time::Duration::from_millis(
								RECONNECT_SLEEP_MS,
							))
							.await;
						},
					}
				}
			}
			let best_block_r = &best_block.read().await.0;
			if self
				.evaluate_block_metrics_and_maybe_stop(
					best_block_r,
					&mut timestamp,
					&mut block_time,
					&mut tps_window,
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

	/// Parse the current best block, update TPS-related metrics, and log a summary.
	/// Returns `true` when TPS drops below a threshold, signaling an early stop.
	async fn evaluate_block_metrics_and_maybe_stop(
		&self,
		best_block_r: &subxt::blocks::Block<PolkadotConfig, OnlineClient<PolkadotConfig>>,
		timestamp: &mut Duration,
		block_time: &mut Duration,
		tps_window: &mut VecDeque<usize>,
	) -> bool {
		let Ok(extrinsics) = best_block_r.extrinsics().await else {
			return false;
		};
		let mut tx_count: u64 = 0;
		for extrinsic in extrinsics.iter() {
			match (
				extrinsic.pallet_name().expect("pallet name"),
				extrinsic.variant_name().expect("variant name"),
			) {
				(PALLET_NAME_TIMESTAMP, EXTRINSIC_VARIANT_NAME_SET) => {
					let new_timestamp = Duration::from_millis(
						codec::Compact::<u64>::decode(&mut &extrinsic.field_bytes()[..])
							.expect("timestamp decodes")
							.0,
					);
					*block_time = new_timestamp.saturating_sub(*timestamp);
					*timestamp = new_timestamp;
				},
				(PALLET_NAME_NFTS, EXTRINSIC_VARIANT_NAME_TRANSFER) => {
					tx_count += 1;
				},
				_ => (),
			}
		}
		for event in best_block_r.events().await.expect("Events are available").iter() {
			let event = event.expect("Event is available");
			if let (PALLET_NAME_BALANCES, EVENT_VARIANT_NAME_TRANSFER) =
				(event.pallet_name(), event.variant_name())
			{
				tx_count += 1;
			}
		}
		self.shared.in_block.fetch_add(tx_count, Ordering::SeqCst);
		let btime_ms = if block_time.is_zero() {
			DEFAULT_BLOCK_TIME_MS
		} else {
			block_time.as_millis() as u64
		};
		let tps_ = tx_count.saturating_mul(1000) / btime_ms.max(1);
		tps_window.push_back(tps_ as usize);
		if tps_window.len() > 12 {
			tps_window.pop_front();
			let avg_tps = tps_window.iter().sum::<usize>();
			if avg_tps < self.cfg.tps / 4 {
				log::warn!("TPS dropped below 25% of target ...");
				return true;
			}
		}
		let avg_tps = tps_window.iter().sum::<usize>() / tps_window.len();
		log::info!(
			"TPS: {} \t | Avg: {} \t | Sent/Exec: {}/{} | Best: {} | txs = {} | block time = {:?}",
			self.cfg.tps,
			avg_tps,
			self.shared.sent.load(Ordering::SeqCst),
			self.shared.in_block.load(Ordering::SeqCst),
			best_block_r.number(),
			tx_count,
			*block_time
		);
		false
	}

	fn spawn_worker(
		&self,
		api: &OnlineClient<PolkadotConfig>,
		i: usize,
		best_block: BestBlockRef,
	) -> tokio::task::JoinHandle<()> {
		let cfg = self.cfg;
		let api = api.clone();
		let shared = self.shared.clone();
		let sender = self.sender_accounts[i].clone();
		let signer = self.sender_signers[i].clone();
		let nrecv = if cfg.batch > 1 { cfg.batch } else { 1 };
		let receivers = self.receiver_accounts[i..i + nrecv].to_vec();
		tokio::spawn(async move {
			tokio::time::sleep(std::time::Duration::from_millis(
				((cfg.n_sender_tasks - i) as u64) * RAMP_SLOT_MS,
			))
			.await;
			let mut state = WorkerState { nonce: 0, sleep_time_ms: 0 };
			let block_ref: BlockRef<subxt::utils::H256> =
				BlockRef::from_hash(best_block.read().await.0.hash());
			state.nonce = fetch_account_nonce_at_block(&api, block_ref.clone(), &sender).await;
			let inputs = WorkerInputs { sender, signer, receivers };
			loop {
				Self::tick(i, &api, &cfg, &shared, &best_block, &inputs, &mut state).await;
			}
		})
	}

	async fn tick(
		i: usize,
		api: &OnlineClient<PolkadotConfig>,
		cfg: &WorkerConfig,
		shared: &SharedState,
		best_block: &BestBlockRef,
		inputs: &WorkerInputs,
		state: &mut WorkerState,
	) {
		if shared.sent.load(Ordering::SeqCst) >
			shared.in_block.load(Ordering::SeqCst) + BACKLOG_THRESHOLD
		{
			tokio::time::sleep(std::time::Duration::from_millis(RETRY_THROTTLE_MS)).await;
			state.sleep_time_ms = state.sleep_time_ms.saturating_sub(RETRY_THROTTLE_MS);
			return;
		}
		tokio::time::sleep(std::time::Duration::from_millis(state.sleep_time_ms)).await;
		let now = Instant::now();
		log::debug!("Sender {} using nonce {}", i, state.nonce);
		let tx_payload = if cfg.batch > 1 {
			let calls = (0..cfg.batch)
				.map(|i| {
					subxt::dynamic::tx(
						PALLET_NAME_BALANCES,
						CALL_NAME_TRANSFER_KEEP_ALIVE,
						vec![
							Value::unnamed_variant(
								"Id",
								[Value::from_bytes(inputs.receivers[i].public())],
							),
							Value::u128(TX_TRANSFER_AMOUNT),
						],
					)
					.into_value()
				})
				.collect::<Vec<_>>();
			subxt::dynamic::tx(
				PALLET_NAME_UTILITY,
				CALL_NAME_BATCH,
				vec![Value::named_composite(vec![("calls", calls.into())])],
			)
		} else {
			subxt::dynamic::tx(
				PALLET_NAME_BALANCES,
				CALL_NAME_TRANSFER_KEEP_ALIVE,
				vec![
					Value::unnamed_variant("Id", [Value::from_bytes(inputs.receivers[0].public())]),
					Value::u128(TX_TRANSFER_AMOUNT),
				],
			)
		};
		let tx_params = Params::new().nonce(state.nonce).build();
		let tx: SubmittableTransaction<_, OnlineClient<_>> = api
			.tx()
			.create_partial_offline(&tx_payload, tx_params)
			.expect("Failed to create partial offline transaction")
			.sign(&inputs.signer);
		match tx.submit_and_watch().await {
			Ok(_watch) => {},
			Err(err) => {
				log::error!("{:?}", err);
				let block_ref: BlockRef<subxt::utils::H256> =
					BlockRef::from_hash(best_block.read().await.0.hash());
				state.nonce = fetch_account_nonce_at_block(api, block_ref, &inputs.sender).await;
				state.sleep_time_ms =
					cfg.worker_sleep.saturating_sub(now.elapsed().as_millis() as u64);
				return;
			},
		};
		shared.sent.fetch_add(cfg.batch as u64, Ordering::SeqCst);
		state.sleep_time_ms = cfg.worker_sleep.saturating_sub(now.elapsed().as_millis() as u64);
		state.nonce += 1;
	}
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
		PALLET_NAME_SYSTEM,
		ENTRY_NAME_ACCOUNT,
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

fn setup_logging() {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);
}

fn main() -> Result<(), Box<dyn Error>> {
	setup_logging();
	let args = Args::parse();
	let app = SenderApp::from(args);
	app.run()
}
