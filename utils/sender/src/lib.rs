use futures::{stream::FuturesUnordered, StreamExt};
use log::*;
use std::{error::Error, time::Duration};
use subxt::{
	config::polkadot::PolkadotExtrinsicParamsBuilder as Params,
	config::substrate::AccountId32,
	dynamic::Value,
	tx::{Signer, SubmittableTransaction},
	OnlineClient, PolkadotConfig,
};
use sp_core::{sr25519::{self, Pair as SrPair}, Pair};
use sp_runtime::{
        traits::{IdentifyAccount, Verify},
        MultiSignature,
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

pub type SignedTx = SubmittableTransaction<PolkadotConfig, OnlineClient<PolkadotConfig>>;

pub fn sign_txs<P, S, C>(
	params: impl Iterator<Item = P>,
	signer: S,
) -> Vec<SignedTx>
where
	P: Send + 'static,
	S: Fn(P) -> SignedTx + Send + Sync + 'static,
	C: subxt::Config,
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

#[derive(Clone)]
pub struct PairSigner {
	account_id: <PolkadotConfig as subxt::Config>::AccountId,
	signer: sr25519::Pair,
}

impl PairSigner {
	/// Creates a new [`Signer`] from an [`sp_core::sr25519::Pair`].
	pub fn new(signer: sr25519::Pair) -> Self {
		let account_id =
			<MultiSignature as Verify>::Signer::from(signer.public()).into_account();
		Self {
			// Convert `sp_core::AccountId32` to `subxt::config::substrate::AccountId32`.
			//
			// This is necessary because we use `subxt::config::substrate::AccountId32` and no
			// From/Into impls are provided between `sp_core::AccountId32` because `polkadot-sdk` isn't a direct
			// dependency in subxt.
			//
			// This can also be done by provided a wrapper type around `subxt::config::substrate::AccountId32` to implement
			// such conversions but that also most likely requires a custom `Config` with a separate `AccountId` type to work
			// properly without additional hacks.
			account_id: AccountId32(account_id.into()),
			signer,
		}
	}

	/// Returns the [`sp_core::sr25519::Pair`] implementation used to construct this.
	pub fn signer(&self) -> &sr25519::Pair {
		&self.signer
	}

	/// Return the account ID.
	pub fn account_id(&self) -> &AccountId32 {
		&self.account_id
	}
}

impl Signer<PolkadotConfig> for PairSigner {
	fn account_id(&self) -> <PolkadotConfig as subxt::Config>::AccountId {
		self.account_id.clone()
	}

	fn sign(&self, signer_payload: &[u8]) -> <PolkadotConfig as subxt::Config>::Signature {
		let signature = self.signer.sign(signer_payload);
		subxt::utils::MultiSignature::Sr25519(signature.0)
	}
}


pub fn sign_balance_transfers(
	api: OnlineClient<PolkadotConfig>,
	pairs: impl Iterator<Item = ((SrPair, u64), SrPair)>,
) -> Vec<SignedTx> {
	sign_txs::<_, _, PolkadotConfig>(pairs, move |((sender, nonce), receiver)| {
		let signer = PairSigner::new(sender);
		let tx_params = Params::new().nonce(nonce).build();
		let tx_call = subxt::dynamic::tx(
			"Balances",
			"transfer_keep_alive",
			vec![
				Value::unnamed_variant("Id", [Value::from_bytes(receiver.public())]),
				Value::u128(1u32.into()),
			],
		);
		api.tx().create_partial_offline(&tx_call, tx_params).expect("Failed to create partial offline transaction").sign(&signer)
	})
}

/// Here the signed extrinsics are submitted.
pub async fn submit_txs(
	txs: Vec<SubmittableTransaction<PolkadotConfig, OnlineClient<PolkadotConfig>>>,
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
				subxt::tx::TxStatus::Broadcasted =>	log::trace!("BROADCASTED"),
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
