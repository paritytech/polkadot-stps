use futures::StreamExt;
use tokio::sync::mpsc::{self, Sender, UnboundedSender};
use std::collections::HashMap;
use clap::ValueEnum;
// use subxt::tx::PairSigner;
use futures::stream::FuturesUnordered;
use clap::Parser;
use jsonrpsee_client_transport::ws::WsTransportClientBuilder;
use jsonrpsee_core::client::Client;
use parity_scale_codec::{Compact, Decode, Encode, MaxEncodedLen};
use serde_json::json;
use std::{cmp::max, error::Error, sync::Arc, time::Duration};
use subxt::{
	backend::legacy::LegacyBackend, config::DefaultExtrinsicParamsBuilder, OnlineClient
};
use sp_core::crypto::{Pair as _, Ss58Codec};
use sp_runtime::traits::IdentifyAccount;
use zombienet_sdk::{NetworkConfigBuilder, NetworkConfigExt, NetworkNode, RegistrationStrategy};
use subxt::dynamic::Value as TxValue;

use sha3::{Keccak256, Digest};
use sp_core::{keccak_256, ecdsa};
use subxt::utils::H160;
use subxt::Config;
use subxt::tx::Signer;
mod metrics;
use metrics::*;

#[derive(
	Eq, PartialEq, Copy, Clone, Encode, Decode, MaxEncodedLen, Default, PartialOrd, Ord, Hash
)]
pub struct AccountId20(pub [u8; 20]);

impl_serde::impl_fixed_hash_serde!(AccountId20, 20);

impl std::fmt::Display for AccountId20 {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let address = hex::encode(self.0).trim_start_matches("0x").to_lowercase();
		let address_hash = hex::encode(keccak_256(address.as_bytes()));

		let checksum: String =
			address
				.char_indices()
				.fold(String::from("0x"), |mut acc, (index, address_char)| {
					let n = u16::from_str_radix(&address_hash[index..index + 1], 16)
						.expect("Keccak256 hashed; qed");

					if n > 7 {
						// make char uppercase if ith character is 9..f
						acc.push_str(&address_char.to_uppercase().to_string())
					} else {
						// already lowercased
						acc.push(address_char)
					}

					acc
				});
		write!(f, "{checksum}")
	}
}

impl core::fmt::Debug for AccountId20 {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		write!(f, "{:?}", H160(self.0))
	}
}

impl From<[u8; 20]> for AccountId20 {
	fn from(bytes: [u8; 20]) -> Self {
		Self(bytes)
	}
}

impl From<AccountId20> for [u8; 20] {
	fn from(value: AccountId20) -> Self {
		value.0
	}
}

impl From<H160> for AccountId20 {
	fn from(h160: H160) -> Self {
		Self(h160.0)
	}
}

impl From<AccountId20> for H160 {
	fn from(value: AccountId20) -> Self {
		H160(value.0)
	}
}

impl std::str::FromStr for AccountId20 {
	type Err = &'static str;
	fn from_str(input: &str) -> Result<Self, Self::Err> {
		H160::from_str(input).map(Into::into).map_err(|_| "invalid hex address.")
	}
}

type EthSignature = [u8; 65];

pub enum MythicalConfig {}

impl Config for MythicalConfig {
    type Hash = subxt::utils::H256;
    type AccountId = AccountId20;
    type Address = AccountId20;
    type Signature = EthSignature;
    type Hasher = subxt::config::substrate::BlakeTwo256;
    type Header = subxt::config::substrate::SubstrateHeader<u32, subxt::config::substrate::BlakeTwo256>;
    type ExtrinsicParams = subxt::config::SubstrateExtrinsicParams<Self>;
    type AssetId = u32;
}

pub struct EthereumSigner {
	account_id: AccountId20,
	signer: ecdsa::Pair,
}

impl sp_runtime::traits::IdentifyAccount for EthereumSigner {
	type AccountId = AccountId20;
	fn into_account(self) -> Self::AccountId {
		self.account_id
	}
}

impl From<ecdsa::Pair> for EthereumSigner
// where C::AccountId: AccountId20
{
	fn from(pair: ecdsa::Pair) -> Self {
		let decompressed = libsecp256k1::PublicKey::parse_compressed(&pair.public().0)
			.expect("Wrong compressed public key provided")
			.serialize();
		let mut m = [0u8; 64];
		m.copy_from_slice(&decompressed[1..65]);
		Self { account_id: H160::from_slice(&Keccak256::digest(m).as_slice()[12..32]).into(),
			signer: pair
		}
	}
}

impl Signer<MythicalConfig> for EthereumSigner {
    fn account_id(&self) -> <MythicalConfig as Config>::AccountId {
        self.account_id
    }

    fn sign(&self, signer_payload: &[u8]) -> <MythicalConfig as Config>::Signature {
        let hash = keccak_256(signer_payload);
        let wrapped = libsecp256k1::Message::parse_slice(&hash).unwrap();
        self.signer.sign_prehashed(&wrapped.0.b32()).try_into().expect("Signature has correct length")
    }
}


/// Default derivation path for pre-funded accounts
const SENDER_SEED: &str = "//Sender";
const RECEIVER_SEED: &str = "//Receiver";
const FUNDS: u64 = 10_000_000_000_000_000_000;

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

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum BenchMode {
	/// Standard balance transfers
	Stps,

	/// NFT transfers
	NftTransfer,
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

	/// Benchmark mode
	#[arg(long, short, value_enum, default_value_t = BenchMode::Stps)]
	mode: BenchMode,

	/// Number of sender and receiver accounts to create. By defauilt, threads*count senders and as many
	/// receivers are created. With this option, that number may be overridden, but it shouldn't be less
	/// than threads*count.
	#[arg(long)]
	accounts: Option<usize>,

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

	/// Chainspec command template.
	#[arg(long)]
	relay_chainspec_command: Option<String>,

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

	/// Args for parachain collator binary.
	#[arg(long)]
	para_args: Option<String>,

	/// Number of collators
	#[arg(long, default_value_t = 1_usize)]
	para_nodes: usize,

	/// Name of the parachain spec or path to the spec file. If not specified, defaults
	/// to the default chain spec of the collator.
	#[arg(long)]
	para_chain: Option<String>,

	/// Path to a custom parachain spec.
	#[arg(long)]
	para_chainspec: Option<String>,

	/// Chainspec command template.
	#[arg(long)]
	para_chainspec_command: Option<String>,

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
					if let Ok(rwerr) = e.downcast::<reqwest::Error>() {
						if rwerr.is_connect() {
							log::debug!("Ignoring connection refused error");
							continue
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

#[derive(Decode)]
struct Collection {
	clid: [u64; 4],
	owner: AccountId20,
}

#[derive(Decode)]
enum FinalizedEvent {
	NftCollectionCreated(Collection),
	NftMinted,
}

async fn block_subscriber(
	api: OnlineClient<MythicalConfig>,
	ntrans: usize,
	coll_sender: Option<UnboundedSender<FinalizedEvent>>,
	metrics: Option<StpsMetrics>,
) -> Result<(), subxt::Error> {
	let mut blocks_sub = api.blocks().subscribe_finalized().await?;

	let mut last_block_timestamp = 0;
	let mut total_blocktime = 0;
	let mut total_ntrans = 0;
	let mut _first_tran_timestamp = 0;
	let mut max_trans = 0;
	let mut max_tps = 0.0;
	log::debug!("Starting chain watcher");
	while let Some(block) = blocks_sub.next().await {
		let block = block?;
		let mut last_block_ntrans = 0;
		let mut last_blocktime: u64 = 0;

		log::trace!(target: "events","BLOCK {}", block.number());

		for ex in block.extrinsics().await?.iter() {
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
				("Balances", "transfer_keep_alive") | ("Nfts", "transfer") => {
					last_block_ntrans += 1;
				},
				_ => (),
			}
		}

		let mut proc_coll = 0;
		let mut proc_mint = 0;
		for ev in block.events().await?.iter() {
			let ev = ev?;
			log::trace!(target: "events","EVENT {}::{}", ev.pallet_name(), ev.variant_name());
			match (ev.pallet_name(), ev.variant_name()) {
				("Nfts", "Created") => {
					proc_coll += 1;
					if let Some(ref sender) = coll_sender {
						let b = ev.field_bytes();
						let c = Collection::decode(&mut &b[..])?;
						sender.send(FinalizedEvent::NftCollectionCreated(c)).expect("Sender sends");
					}
				},
				("Nfts", "Issued") => {
					proc_mint += 1;
					if let Some(ref sender) = coll_sender {
						sender.send(FinalizedEvent::NftMinted).expect("Sender sends");
					}
				},
				_ => ()
			}
		}

		if last_block_ntrans > 0 {
			log::debug!(
				"Last block time {last_blocktime}, {last_block_ntrans} transactions in block"
			);
			total_blocktime += last_blocktime;
			total_ntrans += last_block_ntrans;
			max_trans = max(max_trans, last_block_ntrans);
			let block_tps = last_block_ntrans as f64 / (last_blocktime as f64 / 1_000_f64);
			max_tps = f64::max(max_tps, block_tps);
			log::info!("TPS in block: {:?}", block_tps);
			log::info!(
				"TPS average: {}",
				total_ntrans as f64 / (total_blocktime as f64 / 1_000_f64)
			);
			log::info!("Max TPS: {max_tps}, max transactions per block {max_trans}");
			if let Some(ref metrics) = metrics {
				metrics.set(last_block_ntrans, last_blocktime, block.number());
			}
		}

		if proc_coll > 0 {
			log::info!("Created NFT collections in block: {proc_coll}");
		}

		if proc_mint > 0 {
			log::info!("Minted NFTs in block: {proc_mint}");
		}

		log::info!("Total transactions processed: {total_ntrans}");

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

	let naccs = if let Some(accounts) = args.accounts {
		assert!(accounts >= ntrans, "Number of accounts specified is less than the number of transactions");
		accounts
	} else {
		ntrans
	};

	let mut send_accs: Vec<_> = funder_lib::derive_accounts::<ecdsa::Pair>(ntrans, SENDER_SEED.to_owned());
	let mut recv_accs: Vec<_> = funder_lib::derive_accounts::<ecdsa::Pair>(ntrans, RECEIVER_SEED.to_owned());

	let accs = send_accs
		.iter()
		.chain(recv_accs.iter())
		.map(|p| (format!("{}", EthereumSigner::from(p.clone()).into_account()), FUNDS))
		.collect::<Vec<_>>();

	let genesis_accs = json!({ "balances": { "balances": &serde_json::to_value(accs)? } });

	send_accs.truncate(ntrans);
	recv_accs.truncate(ntrans);

	let mut relay_hostname = HostnameGen::new("validator");

	let relay = NetworkConfigBuilder::new().with_relaychain(|r| {
		let r = r.with_chain(args.relay_chain.as_str());
		let r = if let Some(chainspec) = args.relay_chainspec {
			r.with_chain_spec_path(chainspec.as_str())
		} else if let Some(chainspec_command) = args.relay_chainspec_command {
			r.with_chain_spec_command(chainspec_command)
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

		//let r = if !args.para { r.with_genesis_overrides(genesis_accs.clone()) } else { r };
		let r = r.with_genesis_overrides(json!({ "configuration": { "config": { "executor_params": [ { "MaxMemoryPages": 8192 }, { "PvfExecTimeout": [ "Backing", 2500 ] } ] } } } ));

		let mut r = r.with_node(|node| node.with_name(relay_hostname.next().as_str()).invulnerable(true));

		for _ in 1..args.relay_nodes {
			r = r.with_node(|node| node.with_name(relay_hostname.next().as_str()).invulnerable(true));
		}

		r
	});

	let network = if !args.para {
		relay
	} else {
		let mut para_hostname = HostnameGen::new("collator");
		relay.with_parachain(|p| {
			let p = p.with_id(args.para_id).with_default_command(args.para_bin.as_str()).evm_based(true);
				// .with_chain_spec_command("{{mainCommand}} build-spec --extra-heap-pages 65000 --chain {{chainName}} {{disableBootnodes}}");

			let p = if let Some(chain) = args.para_chain {
				p.with_chain(chain.as_str())
			} else {
				p
			};

			let p = if let Some(chainspec) = args.para_chainspec {
				p.with_chain_spec_path(chainspec.as_str())
			} else if let Some(chainspec_command) = args.para_chainspec_command {
				p.with_chain_spec_command(chainspec_command)
			} else {
				p
			};

			let p = if let Some(para_args) = args.para_args {
				let pairs: Vec<_> = para_args.split(',').collect();
				let mut a = Vec::new();
				for p in pairs {
					a.push(if p.contains('=') {
						let pv: Vec<_> = p.splitn(2, '=').collect();
						(format!("--{}", pv[0]).as_str(), pv[1]).into()
					} else {
						format!("--{p}").as_str().into()
					});
				}
				a.push("--".into());
				p.with_default_args(a)
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

	let node_url = url::Url::parse(node.ws_uri())?;
	let (node_sender, node_receiver) = WsTransportClientBuilder::default().build(node_url).await?;
	let client = Client::builder()
		.request_timeout(Duration::from_secs(3600))
		.max_buffer_capacity_per_subscription(4096 * 1024)
		.max_concurrent_requests(2 * 1024 * 1024)
		.build_with_tokio(node_sender, node_receiver);
	let backend = LegacyBackend::builder().build(client);
	let api = OnlineClient::from_backend(Arc::new(backend)).await?;

	// When using local senders, it is okay to skip pre-conditions check as we've just generated
	// everything ourselves

	let (coll_send, mut coll_recv) = mpsc::unbounded_channel();

	let sub_api = api.clone();
	let subscriber = tokio::spawn(async move {
		match block_subscriber(sub_api, ntrans, Some(coll_send), metrics).await {
			Ok(()) => {
				log::debug!("Block subscriber exited");
			},
			Err(e) => {
				log::error!("Block subscriber exited with error: {:?}", e);
			},
		}
	});

	log::info!("Signing {} transactions...", send_accs.len());
	let txs = match args.mode {
		BenchMode::Stps => {
		// 	sender_lib::sign_balance_transfers(api.clone(), send_accs.into_iter().map(|a| (a, 0)).zip(recv_accs.into_iter()))?
		// 	// let api = api.clone();
		// 	// sender_lib::sign_txs(send_accs.into_iter().zip(recv_accs.into_iter()), move |(sender, receiver)| {
		// 	// 	let signer = EthereumSigner::from(sender);
		// 	// 	let tx_params = DefaultExtrinsicParamsBuilder::<MythicalConfig>::new().nonce(0).build();
		// 	// 	let tx_call = subxt::dynamic::tx(
		// 	// 		"Balances",
		// 	// 		"transfer_keep_alive",
		// 	// 		vec![
		// 	// 			TxValue::from_bytes(&EthereumSigner::from(receiver).into_account().0),
		// 	// 			TxValue::u128(1_000_000_000_000_000_000u128),
		// 	// 		],
		// 	// 	);

		// 	// 	api.tx().create_signed_offline(&tx_call, &signer, tx_params.into())
		// 	// })?
			Vec::new()
		},

		BenchMode::NftTransfer => {
			let api2 = api.clone();
			let create_coll_txs = sender_lib::sign_txs(send_accs.clone().into_iter(), move |sender| {
				let tx_params = DefaultExtrinsicParamsBuilder::new().nonce(0).build();
				let tx_call = subxt::dynamic::tx(
					"Nfts",
					"create",
					vec![
						TxValue::from_bytes(&EthereumSigner::from(sender.clone()).into_account().0),
						TxValue::named_composite(
							vec![
								("settings", TxValue::primitive(0u64.into())),
								("max_supply", TxValue::unnamed_variant("Some", vec![TxValue::u128(100u128)])),
								("mint_settings", TxValue::named_composite(vec![
									("mint_type", TxValue::unnamed_variant("Issuer", vec![])),
									("price", TxValue::unnamed_variant("None", vec![])),
									("start_block", TxValue::unnamed_variant("None", vec![])),
									("end_block", TxValue::unnamed_variant("None", vec![])),
									("default_item_settings", TxValue::primitive(0u64.into())),
									("serial_mint", TxValue::bool(true)),
								])),
							]
						)
					]
				);
				api2.tx().create_partial_offline(&tx_call, tx_params).expect("Transaction created").sign(&EthereumSigner::from(sender))
			});

			let futs = create_coll_txs.iter().map(|tx| tx.submit()).collect::<FuturesUnordered<_>>();
			let _res = futs.collect::<Vec<_>>().await.into_iter().collect::<Result<Vec<_>, _>>().expect("All the transactions submitted successfully");

			let mut proc_coll = 0;
			let mut map: HashMap<AccountId20, [u64; 4]> = HashMap::new();
			while proc_coll < ntrans {
				let e = coll_recv.recv().await.expect("Recv receives");
				let FinalizedEvent::NftCollectionCreated(c) = e else {
					panic!("Unexpected event");
				};
				map.insert(c.owner, c.clid);
				proc_coll += 1;
			}

			let mut cll = Vec::new();

			for s in send_accs.clone().into_iter() {
				let addr = EthereumSigner::from(s.clone()).into_account();
				let cl = map.get(&addr).expect("Collection exists").clone();
				cll.push((s, cl));
			}

			let api2 = api.clone();

			let mint_txs = sender_lib::sign_txs(cll.clone().into_iter(), move |coll| {
				let tx_params = DefaultExtrinsicParamsBuilder::new().nonce(1).build();
				let tx_call = subxt::dynamic::tx(
					"Nfts",
					"mint",
					vec![
						TxValue::unnamed_composite(coll.1.into_iter().map(|a| a.into())),
						TxValue::unnamed_variant("None", vec![]),
						TxValue::from_bytes(&EthereumSigner::from(coll.0.clone()).into_account().0),
						TxValue::unnamed_variant("None", vec![]),
					]
				);
				api2.tx().create_partial_offline(&tx_call, tx_params).expect("Transaction created").sign(&EthereumSigner::from(coll.0))
			});

			let futs = mint_txs.iter().map(|tx| tx.submit()).collect::<FuturesUnordered<_>>();
			let _res = futs.collect::<Vec<_>>().await.into_iter().collect::<Result<Vec<_>, _>>().expect("All the mint transactions submitted successfully");

			let mut proc_mint = 0;
			while proc_mint < ntrans {
				let e = coll_recv.recv().await.expect("Receiver receives");
				if !matches!(e, FinalizedEvent::NftMinted) {
					panic!("Unexpected event");
				}
				proc_mint += 1;
			}

			let api2 = api.clone();

			sender_lib::sign_txs(cll.into_iter().zip(recv_accs.into_iter()), move |(coll, receiver)| {
				let signer = EthereumSigner::from(coll.0);
				let tx_params = DefaultExtrinsicParamsBuilder::<MythicalConfig>::new().nonce(2).build();
				let tx_call = subxt::dynamic::tx(
					"Nfts",
					"transfer",
					vec![
						TxValue::unnamed_composite(coll.1.into_iter().map(|a| a.into())),
						TxValue::u128(1u128),
						TxValue::from_bytes(&EthereumSigner::from(receiver).into_account().0),
					],
				);

				api2.tx().create_partial_offline(&tx_call, tx_params).expect("Transaction created").sign(&signer)
			})
		}
	};


	log::info!("Transactions signed");

	log::info!("Sending transactions...");
	sender_lib::submit_txs(txs).await?;
	log::info!("All sent");

	tokio::try_join!(subscriber)?;
	log::debug!("Block subscriber joined");

	while args.keep {
		tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
	}

	Ok(())
}
