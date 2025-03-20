use frame_system::Config;
use futures::{stream::FuturesUnordered, StreamExt};
use log::*;
use std::{error::Error, time::Duration};
use subxt::{
	config::polkadot::PolkadotExtrinsicParamsBuilder as Params,
	dynamic::Value,
	tx::{Signer, SubmittableTransaction},
	OnlineClient,
};
use sp_core::{sr25519::Pair as SrPair, Pair};

/// Maximal number of connection attempts.
const MAX_ATTEMPTS: usize = 10;
/// Delay period between failed connection attempts.
const RETRY_DELAY: Duration = Duration::from_secs(1);

pub async fn connect<C: subxt::Config>(url: &str) -> Result<OnlineClient<C>, Box<dyn Error>> {
	for i in 0..MAX_ATTEMPTS {
		debug!("Attempt #{}: Connecting to {}", i, url);
		match OnlineClient::<C>::from_url(url).await {
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

pub type SignedTx<C: subxt::Config> = SubmittableTransaction<C, OnlineClient<C>>;

pub fn sign_txs<P, S, C>(
	params: impl Iterator<Item = P>,
	signer: S,
) -> Vec<SignedTx<C>>
where
	C: subxt::Config,
	P: Send + 'static,
	S: Fn(P) -> SignedTx<C> + Send + Sync + 'static,
{
	let t = std::thread::available_parallelism().unwrap_or(1usize.try_into().unwrap()).get();

	let mut tn = (0..t).cycle();
	let mut tranges: Vec<_> = (0..t).map(|_| Vec::new()).collect();
	params.for_each(|p| tranges[tn.next().unwrap()].push(p));
	let mut threads = Vec::new();

	let signer = std::sync::Arc::new(signer);

	tranges.into_iter().for_each(|chunk| {
		// let api = api.clone();
		let signer = signer.clone();
		threads
			.push(std::thread::spawn(move || chunk.into_iter().map(&*signer).collect::<Vec<_>>()));
	});

	threads
		.into_iter()
		.map(|h| h.join().unwrap())
		.flatten()
		.collect()
}

// pub fn sign_balance_transfers<C>(
// 	api: OnlineClient<PolkadotConfig>,
// 	pairs: impl Iterator<Item = ((SrPair, u64), SrPair)>,
// ) -> Result<Vec<SignedTx>, Box<dyn Error>> {
// 	sign_txs(pairs, move |((sender, nonce), receiver)| {
// 		let signer = PairSigner::new(sender);
// 		let tx_params = Params::new().nonce(nonce).build();
// 		let tx_call = subxt::dynamic::tx(
// 			"Balances",
// 			"transfer_keep_alive",
// 			vec![
// 				Value::unnamed_variant("Id", [Value::from_bytes(receiver.public())]),
// 				Value::u128(1u32.into()),
// 			],
// 		);
// 		api.tx().create_signed_offline(&tx_call, &signer, tx_params)
// 	})
// }

/// Here the signed extrinsics are submitted.
pub async fn submit_txs<C: subxt::Config>(
	txs: Vec<SubmittableTransaction<C, OnlineClient<C>>>,
) -> Result<(), Box<dyn Error>> {
	let futs = txs.iter().map(|tx| tx.submit_and_watch()).collect::<FuturesUnordered<_>>();
	let res = futs.collect::<Vec<_>>().await;
	let res: Result<Vec<_>, _> = res.into_iter().collect();
	let res = res.expect("All the transactions submitted successfully");
	let mut statuses = futures::stream::select_all(res);
	while let Some(a) = statuses.next().await {
		match a {
			Ok(st) => match st {
				subxt::tx::TxStatus::Validated => log::trace!("VALIDATED"),
				subxt::tx::TxStatus::Broadcasted =>
					log::trace!("BROADCASTED"),
				subxt::tx::TxStatus::NoLongerInBestBlock => log::warn!("NO LONGER IN BEST BLOCK"),
				subxt::tx::TxStatus::InBestBlock(_) => log::trace!("IN BEST BLOCK"),
				subxt::tx::TxStatus::InFinalizedBlock(_) => log::trace!("IN FINALIZED BLOCK"),
				subxt::tx::TxStatus::Error { message } => log::warn!("ERROR: {message}"),
				subxt::tx::TxStatus::Invalid { message } => log::trace!("INVALID: {message}"),
				subxt::tx::TxStatus::Dropped { message } => log::trace!("DROPPED: {message}"),
			},
			Err(e) => {
				warn!("Error status {:?}", e);
			},
		}
	}
	Ok(())
}
