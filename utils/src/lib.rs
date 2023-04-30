use log::{error, info, warn};
use std::time::Duration;
use subxt::{OnlineClient, PolkadotConfig};

#[subxt::subxt(runtime_metadata_path = "tick-meta.scale")]
pub mod runtime {}

/// Api of the runtime.
pub type Api = OnlineClient<PolkadotConfig>;
/// Error type for the crate.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// Maximal number of connection attempts.
pub const MAX_ATTEMPTS: usize = 10;
/// Delay period between failed connection attempts.
pub const RETRY_DELAY: Duration = Duration::from_secs(1);
/// Default derivation path for pre-funded accounts
pub const DERIVATION: &str = "//Sender/";

/// Tries [`MAX_ATTEMPTS`] times to connect to the given node.
pub async fn connect(url: &str) -> Result<Api, Error> {
	for i in 1..=MAX_ATTEMPTS {
		info!("Attempt #{}: Connecting to {}", i, url);
		let promise = OnlineClient::<PolkadotConfig>::from_url(url);

		match promise.await {
			Ok(client) => return Ok(client),
			Err(err) => {
				warn!("API client {} error: {:?}", url, err);
				tokio::time::sleep(RETRY_DELAY).await;
			},
		}
	}

	let err = format!("Failed to connect to {} after {} attempts", url, MAX_ATTEMPTS);
	error!("{}", err);
	Err(err.into())
}
