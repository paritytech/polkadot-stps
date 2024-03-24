use clap::Parser;
use parity_scale_codec::{Compact, Decode};
use serde_json::json;
use std::{error::Error, time::Duration};
use subxt::{
	ext::sp_core::{crypto::Ss58Codec, Pair},
	OnlineClient, PolkadotConfig,
};
use zombienet_sdk::{NetworkConfigBuilder, NetworkConfigExt, NetworkNode, RegistrationStrategy};

mod metrics;
use metrics::*;

/// Default derivation path for pre-funded accounts
const SENDER_SEED: &str = "//Sender";
const RECEIVER_SEED: &str = "//Receiver";
const FUNDS: u64 = 10_000_000_000_000_000;

struct HostnameGen {
	prefix: String,
	count: usize,
}

impl HostnameGen {
	fn new(prefix: impl Into<String>) -> Self {
		Self { prefix: prefix.into(), count: 0 }
	}

	fn next(&mut self) -> String {
		self.count += 1;
		format!("{}{:02}", self.prefix, self.count)
	}
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
	/// The ss58 prefix to use (https://github.com/paritytech/ss58-registry/blob/main/ss58-registry.json)
	#[arg(long, short, default_value_t = 42_u16)]
	ss58_prefix: u16,

	/// Number of threads to spawn for account deriving, transaction signing and transaction sending.
	/// Defaults to 4. If set to 0, defaults to the available number of CPU cores.
	#[arg(long, short, default_value_t = 4_usize)]
	threads: usize,

	/// Number of transactions PER THREAD. There will be derived threads*count accounts and as much
	/// transactions will be signed and submitted.
	#[arg(long, short, default_value_t = 100_usize)]
	count: usize,

	/// Path to relay chain node binary.
	#[arg(long, default_value = "polkadot")]
	relay_bin: String,

	/// Args for relay chain binary.
	#[arg(long)]
	relay_args: Option<String>,

	/// Name of the relay chain spec.
	#[arg(long, default_value = "rococo-local")]
	relay_chain: String,

	/// Path to a custom relay chain spec.
	#[arg(long)]
	relay_chainspec: Option<String>,

	/// Number of validators.
	#[arg(long, default_value_t = 2_usize)]
	relay_nodes: usize,

	/// Perform TPS benchmark on parachain instead of relay chain.
	#[arg(long, short)]
	para: bool,

	/// Parachain id.
	#[arg(long, default_value_t = 100_u32)]
	para_id: u32,

	/// Path to parachain collator binary.
	#[arg(long, default_value = "polkadot-parachain")]
	para_bin: String,

	/// Number of collators
	#[arg(long, default_value_t = 1_usize)]
	para_nodes: usize,

	/// Name of the parachain spec or path to the spec file. If not specified, defaults
	/// to the default chain spec of the collator.
	#[arg(long)]
	para_chain: Option<String>,

	/// Block height to wait for before starting the benchmark
	#[arg(long, short, default_value_t = 5_usize)]
	block_height: usize,

	/// Keep the network running after the benchmark is finished until it's interrupted manually
	#[arg(long, short)]
	keep: bool,

	/// Prometheus Listener URL
	#[arg(long)]
	prometheus_url: Option<String>,

	/// Prometheus Listener Port
	#[arg(long, default_value_t = 65432)]
	prometheus_port: u16,
}

async fn wait_for_metric(
	node: &NetworkNode,
	metric: impl Into<String> + Copy,
	timeout: Duration,
	predicate: impl Fn(f64) -> bool + Copy,
) -> Result<(), Box<dyn Error>> {
	tokio::time::timeout(timeout, async {
		loop {
			tokio::time::sleep(std::time::Duration::from_secs(6)).await;
			log::trace!("Checking metric");
			match node.assert_with(metric, predicate).await {
				Ok(r) =>
					if r {
						return Ok(());
					},
				Err(e) => {
					let cause = e.to_string();
					if let Ok(ioerr) = e.downcast::<std::io::Error>() {
						if ioerr.kind() == std::io::ErrorKind::ConnectionRefused {
							log::debug!("Ignoring connection refused error");
							// The node is not ready to accept connections yet
							continue;
						}
					}
					panic!("Cannot assert on node metric: {:?}", cause)
				},
			}
		}
	})
	.await?
}

fn looks_like_filename<'a>(v: impl Into<&'a str>) -> bool {
	let v: &str = v.into();
	v.contains(".") || v.contains("/")
}

async fn block_subscriber(
	api: OnlineClient<PolkadotConfig>,
	ntrans: usize,
	metrics: Option<StpsMetrics>,
) -> Result<(), subxt::Error> {
	let mut blocks_sub = api.blocks().subscribe_finalized().await?;

	let mut last_block_timestamp = 0;
	let mut total_blocktime = 0;
	let mut total_ntrans = 0;
	let mut _first_tran_timestamp = 0;
	log::debug!("Starting chain watcher");
	while let Some(block) = blocks_sub.next().await {
		let block = block?;
		let mut last_block_ntrans = 0;
		let mut last_blocktime: u64 = 0;

		for ex in block.extrinsics().await?.iter() {
			let ex = ex?;
			match (ex.pallet_name()?, ex.variant_name()?) {
				("Timestamp", "set") => {
					let timestamp: Compact<u64> = Decode::decode(&mut &ex.field_bytes()[..])?;
					let timestamp = u64::from(timestamp);
					last_blocktime = timestamp - last_block_timestamp;
					if total_ntrans == 0 {
						_first_tran_timestamp = timestamp;
					}
					last_block_timestamp = timestamp;
				},
				("Balances", "transfer_keep_alive") => {
					last_block_ntrans += 1;
				},
				_ => (),
			}
		}

		if last_block_ntrans > 0 {
			total_blocktime += last_blocktime;
			total_ntrans += last_block_ntrans;
			let block_tps = last_block_ntrans as f64 / (last_blocktime as f64 / 1_000_f64);
			log::info!("TPS in block: {:?}", block_tps);
			log::info!(
				"TPS average: {}",
				total_ntrans as f64 / (total_blocktime as f64 / 1_000_f64)
			);
			if let Some(ref metrics) = metrics {
				metrics.set(last_block_ntrans, last_blocktime, block.number());
			}
		}

		if total_ntrans >= ntrans as u64 {
			break;
		}
	}
	Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	let args = Args::parse();

	let nthreads = if args.threads == 0 {
		std::thread::available_parallelism().unwrap_or(1usize.try_into().unwrap()).get()
	} else {
		args.threads
	};
	let ntrans = nthreads * args.count;

	let send_accs = funder_lib::derive_accounts(ntrans, SENDER_SEED.to_owned());
	let recv_accs = funder_lib::derive_accounts(ntrans, RECEIVER_SEED.to_owned());

	let accs = send_accs
		.iter()
		.chain(recv_accs.iter())
		.map(|p| (p.public().to_ss58check_with_version(args.ss58_prefix.into()), FUNDS))
		.collect::<Vec<_>>();

	let genesis_accs = json!({ "balances": { "balances": &serde_json::to_value(accs)? } });

	let mut relay_hostname = HostnameGen::new("validator");

	let relay = NetworkConfigBuilder::new().with_relaychain(|r| {
		let r = r.with_chain(args.relay_chain.as_str());
		let r = if let Some(chainspec) = args.relay_chainspec {
			r.with_chain_spec_path(chainspec.as_str())
		} else {
			r
		};
		let r = r.with_default_command(args.relay_bin.as_str());

		let r = if let Some(relay_args) = args.relay_args {
			let pairs: Vec<_> = relay_args.split(',').collect();
			let mut a = Vec::new();
			for p in pairs {
				a.push(if p.contains('=') {
					let pv: Vec<_> = p.splitn(2, '=').collect();
					(format!("--{}", pv[0]).as_str(), pv[1]).into()
				} else {
					format!("--{p}").as_str().into()
				});
			}
			r.with_default_args(a)
		} else {
			r
		};

		let r = if !args.para { r.with_genesis_overrides(genesis_accs.clone()) } else { r };

		let mut r = r.with_node(|node| node.with_name(relay_hostname.next().as_str()));

		for _ in 1..args.relay_nodes {
			r = r.with_node(|node| node.with_name(relay_hostname.next().as_str()));
		}

		r
	});

	let network = if !args.para {
		relay
	} else {
		let mut para_hostname = HostnameGen::new("collator");
		relay.with_parachain(|p| {
			let p = p.with_id(args.para_id).with_default_command(args.para_bin.as_str());

			let p = if let Some(chain) = args.para_chain {
				let chain = chain.as_str();
				if looks_like_filename(chain) {
					p.with_chain_spec_path(chain)
				} else {
					p.with_chain(chain)
				}
			} else {
				p
			};
			let mut p = p
				.cumulus_based(true)
				.with_registration_strategy(RegistrationStrategy::InGenesis)
				.with_genesis_overrides(genesis_accs)
				.with_collator(|n| n.with_name(para_hostname.next().as_str()));

			for _ in 1..args.para_nodes {
				p = p.with_collator(|n| n.with_name(para_hostname.next().as_str()));
			}

			p
		})
	};

	let network = network.build().unwrap();
	let network = network.spawn_native().await?;

	let metrics = if let Some(url) = args.prometheus_url {
		Some(run_prometheus_endpoint(&url, &args.prometheus_port).await?)
	} else {
		None
	};

	let node = network.get_node(if args.para { "collator01" } else { "validator01" })?;

	wait_for_metric(node, "block_height{status=\"best\"}", Duration::from_secs(300), |bh| {
		bh >= args.block_height as f64
	})
	.await?;

	log::info!("Block height reached");
	let api = node.client().await?;

	log::info!("Signing transactions...");
	let txs = sender_lib::sign_txs(api.clone(), send_accs.into_iter().zip(recv_accs.into_iter()))?;
	log::info!("Transactions signed");

	// When using local senders, it is okay to skip pre-conditions check as we've just generated
	// everything ourselves

	let subscriber = tokio::spawn(async move {
		match block_subscriber(api.clone(), ntrans, metrics).await {
			Ok(()) => {
				log::debug!("Block subscriber exited");
			},
			Err(e) => {
				log::error!("Block subscriber exited with error: {:?}", e);
			},
		}
	});

	log::info!("Sending transactions...");
	sender_lib::submit_txs(txs, 50).await?;
	log::info!("All sent");

	tokio::try_join!(subscriber)?;
	log::debug!("Block subscriber joined");

	while args.keep {
		tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
	}

	Ok(())
}
