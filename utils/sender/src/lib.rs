
use std::time::Duration;

use futures::{stream::FuturesUnordered, StreamExt};
use log::*;
use std::error::Error;
use subxt::{
	tx::SubmittableExtrinsic,
	OnlineClient, PolkadotConfig,
};

/// Maximal number of connection attempts.
const MAX_ATTEMPTS: usize = 10;
/// Delay period between failed connection attempts.
const RETRY_DELAY: Duration = Duration::from_secs(1);

pub async fn connect(url: &str) -> Result<OnlineClient<PolkadotConfig>, Box<dyn Error>> {
	for i in 0..MAX_ATTEMPTS {
		debug!("Attempt #{}: Connecting to {}", i, url);
		match OnlineClient::<PolkadotConfig>::from_url(url).await {
			Ok(client) => {
				debug!("Connection established to: {}", url);
				return Ok(client);
			},
			Err(err) => {
				warn!("API client {} error: {:?}", url, err);
				tokio::time::sleep(RETRY_DELAY).await;
			},
		};
	}

	let err = format!("Failed to connect to {} after {} attempts", url, MAX_ATTEMPTS);
	error!("{}", err);
	Err(err.into())
}

pub type SignedTx<C> = SubmittableExtrinsic<C, OnlineClient<C>>;

pub fn sign_txs<P, S, E, C>(
	// api: OnlineClient<PolkadotConfig>,
	params: impl Iterator<Item = P>,
	signer: S,
	// txs: impl Iterator<Item = (SrPair, SrPair)>,
) -> Result<Vec<SignedTx<C>>, E>
where
	P: Send + 'static,
	S: Fn(P) -> Result<SignedTx<C>, E> + Send + Sync + 'static,
	E: Error + Send + 'static,
	C: subxt::Config,
{
	let t = std::thread::available_parallelism().unwrap_or(1usize.try_into().unwrap()).get();

	let mut tn = (0..t).cycle();
	let mut tranges: Vec<_> = (0..t).map(|_| Vec::new()).collect();
	params.for_each(|p| tranges[tn.next().unwrap()].push(p));
	let mut threads = Vec::new();

	let signer = std::sync::Arc::new(signer);

	tranges.into_iter().for_each(|chunk| {
		let signer = signer.clone();
		threads.push(std::thread::spawn(move || {
			chunk
				.into_iter()
				.map(&*signer)
				.collect::<Vec<_>>()
		}));
	});

	Ok(threads
		.into_iter()
		.map(|h| h.join().unwrap())
		.flatten()
		.collect::<Result<Vec<_>, _>>()?)
}

// pub fn sign_balance_transfers<P, C>(api: OnlineClient<C>, pairs: impl Iterator<Item = (P, P)>) -> Result<Vec<SignedTx<C>>, Box<dyn Error>>
// where 
// 	MultiSigner: From<<P as Pair>::Public>,
// 	subxt::utils::MultiSignature: From<<P as Pair>::Signature>,
// 	P: Pair + std::marker::Send + 'static,
// 	C: subxt::Config, <C as subxt::Config>::Signature: From<<P as subxt::ext::sp_core::Pair>::Signature>,
// 	<C as subxt::Config>::AccountId: From<subxt::utils::AccountId32>,
// 	C::ExtrinsicParams: From<SubstrateExtrinsicParams<C>>, 
// 	<<C as subxt::Config>::ExtrinsicParams as subxt::config::ExtrinsicParams<C>>::Params: From<((), (), subxt::config::signed_extensions::CheckNonceParams, (), subxt::config::signed_extensions::CheckMortalityParams<C>, subxt::config::signed_extensions::ChargeAssetTxPaymentParams<C>, subxt::config::signed_extensions::ChargeTransactionPaymentParams)>
// {
// 	sign_txs(pairs, move |(sender, receiver)| {
// 		let signer = PairSigner::new(sender);
// 		let tx_params = DefaultExtrinsicParamsBuilder::<C>::new().nonce(0).build();
// 		let tx_call = subxt::dynamic::tx(
// 			"Balances",
// 			"transfer_keep_alive",
// 			vec![
// 				Value::unnamed_variant("Id", [Value::from_bytes(receiver.public())]),
// 				Value::u128(1u32.into()),
// 			],
// 		);
// 		api.tx().create_signed_offline(&tx_call, &signer, tx_params.into())
// 	})
// }

/// Here the signed extrinsics are submitted.
pub async fn submit_txs<C: subxt::Config>(
	txs: Vec<SignedTx<C>>,
) -> Result<(), Box<dyn Error>> {
	let futs = txs.iter().map(|tx| tx.submit_and_watch()).collect::<FuturesUnordered<_>>();
	let res = futs.collect::<Vec<_>>().await;
	let res: Result<Vec<_>, _> = res.into_iter().collect();
	let res = res.expect("All the transactions submitted successfully");
	let mut statuses = futures::stream::select_all(res);
	while let Some(a) = statuses.next().await {
		match a {
			Ok(st) => match st {
				subxt::tx::TxStatus::Validated => log::info!("VALIDATED"),
				subxt::tx::TxStatus::Broadcasted { num_peers } =>
					log::info!("BROADCASTED TO {num_peers}"),
				subxt::tx::TxStatus::NoLongerInBestBlock => log::warn!("NO LONGER IN BEST BLOCK"),
				subxt::tx::TxStatus::InBestBlock(_) => log::info!("IN BEST BLOCK"),
				subxt::tx::TxStatus::InFinalizedBlock(_) => log::info!("IN FINALIZED BLOCK"),
				subxt::tx::TxStatus::Error { message } => log::warn!("ERROR: {message}"),
				subxt::tx::TxStatus::Invalid { message } => log::warn!("INVALID: {message}"),
				subxt::tx::TxStatus::Dropped { message } => log::warn!("DROPPED: {message}"),
			},
			Err(e) => {
				warn!("Error status {:?}", e);
			},
		}
	}
	Ok(())
}
