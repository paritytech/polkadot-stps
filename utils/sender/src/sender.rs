use codec::Encode;
use futures::{stream::FuturesUnordered, StreamExt};
use clap::Parser;
use clap::ValueEnum;
use codec::Decode;
use log::*;
use sender_lib::{connect, sign_balance_transfers};
use sp_core::U256;
use std::sync::Mutex;
use std::{collections::HashMap, error::Error};
use subxt::OnlineClient;
use sp_core::{ecdsa, Pair};
use stps_config::eth::{AccountId20, EthereumSigner, MythicalConfig};
use subxt::config::DefaultExtrinsicParamsBuilder;
use subxt::dynamic::Value as TxValue;
use subxt::tx::Signer;
use sp_runtime::traits::IdentifyAccount;
use tokio::sync::mpsc::{self, UnboundedSender};
const SENDER_SEED: &str = "//Sender";
const RECEIVER_SEED: &str = "//Receiver";

#[derive(Parser, Debug, Clone, Copy, ValueEnum)]
// #[value(rename_all = "kebab-case")]
enum Mode {
	/// Transfer native token balances
	Balance,
	/// Transfer NFTs
	NftTransfer,
    /// Create marketplace orders
    Marketplace,
}

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

	/// Mode of operation - either balance transfers or NFT transfers
	#[arg(long, value_enum, default_value_t = Mode::Balance)]
	mode: Mode,
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

#[derive(Decode)]
struct Collection {
	clid: [u64; 4],
	owner: AccountId20,
}

#[derive(Decode)]
struct Mint {
	clid: [u64; 4],
	item_id: u128,
	owner: AccountId20,
}

#[derive(Decode)]
enum FinalizedEvent {
	NftCollectionCreated(Collection),
	NftMinted(Mint),
}

async fn block_subscriber(
	api: OnlineClient<MythicalConfig>,
	ntrans: usize,
	coll_sender: Option<UnboundedSender<FinalizedEvent>>,
	// metrics: Option<StpsMetrics>,
) -> Result<(), subxt::Error> {
	let mut blocks_sub = api.blocks().subscribe_finalized().await?;

	// let mut last_block_timestamp = 0;
	// let mut total_blocktime = 0;
	// let mut total_ntrans = 0;
	// let mut _first_tran_timestamp = 0;
	// let mut max_trans = 0;
	// let mut max_tps = 0.0;
	log::debug!("Starting chain watcher");
	while let Some(block) = blocks_sub.next().await {
		let block = block?;
		// let mut last_block_ntrans = 0;
		// let mut last_blocktime: u64 = 0;

		// log::trace!(target: "events","BLOCK {}", block.number());

		// for ex in block.extrinsics().await?.iter() {
		// 	match (ex.pallet_name()?, ex.variant_name()?) {
		// 		("Timestamp", "set") => {
		// 			let timestamp: Compact<u64> = Decode::decode(&mut &ex.field_bytes()[..])?;
		// 			let timestamp = u64::from(timestamp);
		// 			last_blocktime = timestamp - last_block_timestamp;
		// 			if total_ntrans == 0 {
		// 				_first_tran_timestamp = timestamp;
		// 			}
		// 			last_block_timestamp = timestamp;
		// 		},
		// 		("Balances", "transfer_keep_alive") | ("Nfts", "transfer") => {
		// 			last_block_ntrans += 1;
		// 		},
		// 		_ => (),
		// 	}
		// }

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
						let b = ev.field_bytes();
						let m = Mint::decode(&mut &b[..])?;
						sender.send(FinalizedEvent::NftMinted(m)).expect("Sender sends");
					}
				},
				_ => ()
			}
		}

		// if last_block_ntrans > 0 {
		// 	log::debug!(
		// 		"Last block time {last_blocktime}, {last_block_ntrans} transactions in block"
		// 	);
		// 	total_blocktime += last_blocktime;
		// 	total_ntrans += last_block_ntrans;
		// 	max_trans = max(max_trans, last_block_ntrans);
		// 	let block_tps = last_block_ntrans as f64 / (last_blocktime as f64 / 1_000_f64);
		// 	max_tps = f64::max(max_tps, block_tps);
		// 	log::info!("TPS in block: {:?}", block_tps);
		// 	log::info!(
		// 		"TPS average: {}",
		// 		total_ntrans as f64 / (total_blocktime as f64 / 1_000_f64)
		// 	);
		// 	log::info!("Max TPS: {max_tps}, max transactions per block {max_trans}");
		// 	if let Some(ref metrics) = metrics {
		// 		metrics.set(last_block_ntrans, last_blocktime, block.number());
		// 	}
		// }

		if proc_coll > 0 {
			log::info!("Created NFT collections in block: {proc_coll}");
		}

		if proc_mint > 0 {
			log::info!("Minted NFTs in block: {proc_mint}");
		}

		// log::info!("Total transactions processed: {total_ntrans}");

		// if total_ntrans >= ntrans as u64 {
		// 	break;
		// }
	}
	Ok(())
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
                    TxValue::from_bytes(EthereumSigner::from(acc.clone()).account_id().0),
                    TxValue::u128(300 * ED),
                ],
            ).into_value()
        ).collect::<Vec<_>>();
        batch_calls.extend(receiver_accounts.iter().skip(i * BATCH_BY).take(BATCH_BY).map(|acc| 
            subxt::dynamic::tx(
                "Balances",
                "transfer_keep_alive",
                vec![
                    TxValue::from_bytes(EthereumSigner::from(acc.clone()).account_id().0),
                    TxValue::u128(300 * ED),
                ],
            ).into_value()
        ));
        let batch = subxt::dynamic::tx(
            "Utility",
            "batch",
            vec![ TxValue::named_composite(vec![("calls", batch_calls.into())]) ]
        );

        let tx_params = DefaultExtrinsicParamsBuilder::new().nonce(alith_nonce).build();
        alith_nonce += 1;
        api.tx().create_partial_offline(&batch, tx_params).unwrap().sign(&alith_signer)
    }).collect::<Vec<_>>();

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

			(account_id, nonce)
		}
	}).collect::<FuturesUnordered<_>>();
	let mut noncemap = futs.collect::<Vec<_>>().await.into_iter().collect::<HashMap<_, _>>();

	let elapsed = now.elapsed();
	info!("Got nonces in {:?}", elapsed);

    let sub_api = api.clone();
    let (coll_send, mut coll_recv) = mpsc::unbounded_channel();
    let subscriber = tokio::spawn(async move {
        match block_subscriber(sub_api, n_tx_sender, Some(coll_send)).await {
            Ok(()) => {
                log::debug!("Block subscriber exited");
            },
            Err(e) => {
                log::error!("Block subscriber exited with error: {:?}", e);
            },
        }
    });

    let now = std::time::Instant::now();
    let txs = if matches!(args.mode, Mode::Balance) {
        sign_balance_transfers(api, sender_accounts.into_iter().map(|sa| (sa.clone(), noncemap[&EthereumSigner::from(sa).account_id()] as u64)).zip(receiver_accounts.into_iter()))
    } else {

        fn accs_with_nonces<'a>(noncemap: &'a mut HashMap<AccountId20, u64>, accs: impl Iterator<Item = ecdsa::Pair> + 'a) -> impl Iterator<Item = (ecdsa::Pair, u64)> + 'a {
            accs.map(|a| {
                let nonce = noncemap.get_mut(&EthereumSigner::from(a.clone()).account_id()).expect("Nonce exists");
                let old = *nonce;
                *nonce += 1;
                (a, old)
            })
        }

        let api2 = api.clone();
        let create_coll_txs = sender_lib::sign_txs(
            //sender_accounts.clone().into_iter(), 
            accs_with_nonces(&mut noncemap, sender_accounts.clone().into_iter()),
            move |(sender, nonce)| {
            let tx_params = DefaultExtrinsicParamsBuilder::new().nonce(nonce).build();
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
        while proc_coll < n_tx_sender {
            let e = coll_recv.recv().await.expect("Recv receives");
            let FinalizedEvent::NftCollectionCreated(c) = e else {
                panic!("Unexpected event");
            };
            map.insert(c.owner, c.clid);
            proc_coll += 1;
        }

        let mut cll = Vec::new();

        for s in accs_with_nonces(&mut noncemap, sender_accounts.clone().into_iter()) {
            let addr = EthereumSigner::from(s.0.clone()).into_account();
            let cl = map.get(&addr).expect("Collection exists").clone();
            cll.push((s.0, cl, s.1));
        }

        let api2 = api.clone();

        let mint_txs = sender_lib::sign_txs(cll.clone().into_iter(), move |coll| {
            let tx_params = DefaultExtrinsicParamsBuilder::new().nonce(coll.2).build();
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
        // let mut mint_map: HashMap<AccountId20, ([u64; 4], u128)> = HashMap::new();
        while proc_mint < n_tx_sender {
            let e = coll_recv.recv().await.expect("Receiver receives");
            if !matches!(e, FinalizedEvent::NftMinted(_)) {
                panic!("Unexpected event");
            }
            // match e {
            //     FinalizedEvent::NftMinted(m) => {
            //         mint_map.insert(m.owner, (m.clid, m.item_id));
            //     }
            //     _ => {
            //         panic!("Unexpected event");
            //     }
            // }
            proc_mint += 1;
        }

        // let mut mints = Vec::new();
        // for s in accs_with_nonces(&mut noncemap, sender_accounts.clone().into_iter()) {
        //     let mint = mint_map.get(&EthereumSigner::from(s.0.clone()).into_account()).expect("Mint exists");
        //     mints.push((s.0, mint.0, mint.1, s.1));
        // }

        // info!("Minted {} NFTs", mints.len());

        let api2 = api.clone();

        if matches!(args.mode, Mode::NftTransfer) {
            sender_lib::sign_txs(cll.into_iter().zip(receiver_accounts.into_iter()), move |(coll, receiver)| {
                let signer = EthereumSigner::from(coll.0);
                let tx_params = DefaultExtrinsicParamsBuilder::<MythicalConfig>::new().nonce(coll.2 + 1).build();
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
        } else {
            use rand::distr::{Alphanumeric, SampleString};

            fn myth_timestamp_now() -> u64 {
                use std::time::{SystemTime, UNIX_EPOCH};
                let duration = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system time");
                duration.as_millis() as u64
            }

            #[derive(Encode)]
            pub struct OrderMessage {
                pub collection: U256,
                pub item: u128,
                pub price: u128,
                pub expires_at: u64,
                pub fee: u128,
                pub escrow_agent: Option<AccountId20>,
                pub nonce: String,
            }

            let fee_signer = EthereumSigner::from(ecdsa::Pair::from_seed(&subxt_signer::eth::dev::faith().secret_key()));
            let fee_signer2 = fee_signer.clone();
            let api2 = api.clone();

            let ask_txs = sender_lib::sign_txs(cll.clone().into_iter(), move |coll| {
                let order_nonce: String = Alphanumeric.sample_string(&mut rand::rng(), 9);
                let expires_at = myth_timestamp_now() + 9_000_001;

                let order_msg = OrderMessage {
                    collection: U256(coll.1),
                    item: 1u128,
                    price: 1u128,
                    expires_at,
                    fee: 1u128,
                    escrow_agent: None,
                    nonce: order_nonce.clone(),
                };
                let order_bytes = order_msg.encode();
                let signature = fee_signer.sign(&order_bytes[..]);
                let tx_params = DefaultExtrinsicParamsBuilder::new().nonce(coll.2 + 1).build();
                let tx_call = subxt::dynamic::tx(
                    "Marketplace",
                    "create_order",
                    vec![ 
                        //TxValue::named_composite(vec![
                            ("order", TxValue::named_composite(vec![
                                ("order_type", TxValue::unnamed_variant("Ask", vec![])),
                                ("collection", TxValue::unnamed_composite(coll.1.into_iter().map(|a| a.into()))),
                                ("item", TxValue::u128(1u128)),
                                ("price", TxValue::u128(1u128)),
                                ("expires_at", TxValue::primitive(expires_at.into())),
                                ("fee", TxValue::u128(1u128)),
                                ("escrow_agent", TxValue::unnamed_variant("None", vec![])),
                                ("signature_data", TxValue::named_composite(vec![
                                    ("signature", TxValue::from_bytes(&signature)),
                                    ("nonce", TxValue::from_bytes(Vec::from(order_nonce))),
                                ])),
                            ])),
                            ("execution", TxValue::unnamed_variant("AllowCreation", vec![])),
                        //]),
                    ]);
                api2.tx().create_partial_offline(&tx_call, tx_params).expect("Transaction created").sign(&EthereumSigner::from(coll.0))
            });

            info!("Submitting ask order transactions");
            let futs = ask_txs.iter().map(|tx| tx.submit_and_watch()).collect::<FuturesUnordered<_>>();
            let submitted = futs.collect::<Vec<_>>().await.into_iter().collect::<Result<Vec<_>, _>>().expect("All the ask order transactions submitted successfully");
            let res = submitted.into_iter().map(|tx| tx.wait_for_finalized()).collect::<FuturesUnordered<_>>();
            let _ = res.collect::<Vec<_>>().await.into_iter().collect::<Result<Vec<_>, _>>().expect("All the ask order transactions finalized successfully");
            info!("Ask order transactions finalized");

            let futs = receiver_accounts.iter().map(|a| {
                let account_id = EthereumSigner::from(a.clone()).account_id();
                let fapi = api.clone();
                async move {
                    let nonce = get_nonce(&fapi, account_id).await;

                    (a.clone(), nonce)
                }
            }).collect::<FuturesUnordered<_>>();
            let recv_noncemap = futs.collect::<Vec<_>>().await; //.into_iter().collect::<HashMap<_, _>>();
            info!("Got receiver nonces, sending bid orders");

            let api2 = api.clone();

            sender_lib::sign_txs(cll.clone().into_iter().zip(recv_noncemap.into_iter()), move |((_, clid, _), (buyer, nonce))| {
                let order_nonce: String = Alphanumeric.sample_string(&mut rand::rng(), 9);
                let expires_at = myth_timestamp_now() + 9_000_001;

                let order_msg = OrderMessage {
                    collection: U256(clid),
                    item: 1u128,
                    price: 1u128,
                    expires_at,
                    fee: 1u128,
                    escrow_agent: None,
                    nonce: order_nonce.clone(),
                };
                let order_bytes = order_msg.encode();
                let signature = fee_signer2.sign(&order_bytes[..]);
                let tx_params = DefaultExtrinsicParamsBuilder::new().nonce(nonce).build();
                let tx_call = subxt::dynamic::tx(
                    "Marketplace",
                    "create_order",
                    vec![ 
                        //TxValue::named_composite(vec![
                            ("order", TxValue::named_composite(vec![
                                ("order_type", TxValue::unnamed_variant("Bid", vec![])),
                                ("collection", TxValue::unnamed_composite(clid.into_iter().map(Into::into))),
                                ("item", TxValue::u128(1u128)),
                                ("price", TxValue::u128(1u128)),
                                ("expires_at", TxValue::primitive(expires_at.into())),
                                ("fee", TxValue::u128(1u128)),
                                ("escrow_agent", TxValue::unnamed_variant("None", vec![])),
                                ("signature_data", TxValue::named_composite(vec![
                                    ("signature", TxValue::from_bytes(&signature)),
                                    ("nonce", TxValue::from_bytes(Vec::from(order_nonce))),
                                ])),
                            ])),
                            ("execution", TxValue::unnamed_variant("Force", vec![])),
                        //]),
                    ]);
                api2.tx().create_partial_offline(&tx_call, tx_params).expect("Transaction created").sign(&EthereumSigner::from(buyer))
            })



            // info!("Submitting ask order transactions");
            // let futs = ask_txs.iter().map(|tx| {
            //     info!("Submitting ask order transaction {:?}", tx.encoded());
            //     tx.submit()
            // }).collect::<FuturesUnordered<_>>();
            // info!("Collected ask order transactions");
            // let _res = futs.collect::<Vec<_>>().await.into_iter().collect::<Result<Vec<_>, _>>().expect("All the ask order transactions submitted successfully");
            // info!("Submitted ask order transactions");
            // todo!();
            // bid_txs
        }
    };
	let elapsed = now.elapsed();
	info!("Signed in {:?}", elapsed);

	info!("Starting sender");

    sender_lib::submit_txs(txs).await?;

	tokio::try_join!(subscriber)?;
	log::debug!("Block subscriber joined");

    Ok(())
}
