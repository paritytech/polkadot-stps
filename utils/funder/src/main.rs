use clap::Parser;
use futures::future::try_join_all;
use serde_json::Value;
use sp_core::{crypto::Ss58Codec, sr25519::Pair as SrPair, Pair};
use sp_runtime::{traits::IdentifyAccount, AccountId32};
use std::{
	fs::File,
	io::Read,
	ops::Range,
	path::{Path, PathBuf},
};

use utils::{Error, DERIVATION};

const DEFAULT_FUNDED_JSON_PATH: &str = "funded-accounts.json";

/// util program to derive pre-funded accounts
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
	/// The number of pre-funded accounts to derive
	#[arg(short, default_value_t = 500000)]
	n: usize,

	/// The ss58 prefix to use (https://github.com/paritytech/ss58-registry/blob/main/ss58-registry.json)
	#[arg(long, short, default_value_t = 42_u16)]
	ss58_prefix: u16,

	/// Path to write the funded accounts to.
	#[arg(long, short, default_value = DEFAULT_FUNDED_JSON_PATH)]
	output: PathBuf,

	/// Number of threads to derive accounts with.
	#[arg(long, short, default_value = "4")]
	threads: usize,
}

/// Initial funds for a genesis account.
const FUNDS: u64 = 10_000_000_000_000_000;

/// Creates a json file with a specific number of accounts.
///
/// * `derivation_blueprint` - An index will be appended to this and used as derivation path.
/// * `n` - The minimum number of accounts to create. The order of accounts is unspecified.
/// * `ss58_prefix` - The prefix to use to generate the json file.
/// * `path` - The path to write the JSON file to.
/// * `threads` - The number of threads to use for deriving the accounts.
pub async fn funded_accounts_json(
	n: usize,
	ss58_prefix: u16,
	path: &Path,
	threads: usize,
) -> Result<(), Error> {
	let accounts = derive_accounts_json(n, ss58_prefix, threads).await?;

	let mut file = File::create(path)?;
	serde_json::to_writer_pretty(&mut file, &accounts).map_err(Into::into)
}

pub async fn derive_accounts_json(
	n: usize,
	ss58_prefix: u16,
	threads: usize,
) -> Result<Value, Error> {
	let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(threads).build()?;
	// Round n up to the next multiple of threads.
	let n = (n + threads - 1) / threads * threads;
	let per_thread = n / threads;
	log::info!("Deriving {} accounts on {} threads; {} per thread.", n, threads, per_thread);
	let now = std::time::Instant::now();

	let mut futures = Vec::new();
	// Spawn `threads` many tasks each with `per_thread` many accounts.
	for i in 0..threads {
		let start = i * per_thread;
		let end = start + per_thread;

		let f = rt.spawn(async move { derive_accounts(start..end).await });
		futures.push(f);
	}

	let funded_accounts: Vec<(String, u64)> = try_join_all(futures)
		.await
		.iter()
		.flatten()
		.flatten()
		.map(|a| (a.to_ss58check_with_version(ss58_prefix.into()), FUNDS))
		.collect();
	// Don't just drop a tokio runtime in async context, since that panics.
	rt.shutdown_background();
	let elapsed = now.elapsed();
	log::info!(
		"Derived  {} accounts in {:.2} seconds; {:.2} per second.",
		n,
		elapsed.as_secs(),
		n as f64 / elapsed.as_secs_f64()
	);

	serde_json::to_value(&funded_accounts).map_err(Into::into)
}

async fn derive_accounts(range: Range<usize>) -> Vec<AccountId32> {
	range
		.map(|i| {
			let derivation = format!("{}{}", DERIVATION, i);
			let pair: SrPair = Pair::from_string(&derivation, None).unwrap();
			pair.public().into_account().into()
		})
		.collect()
}

/// Returns the number of accounts in the `funded-accounts.json` file.
pub fn n_accounts(json_path: &PathBuf) -> usize {
	let mut file = File::open(json_path).unwrap();
	let mut json_bytes = Vec::new();
	file.read_to_end(&mut json_bytes).expect("Unable to read data");

	let json: Value = serde_json::from_slice(&json_bytes).unwrap();
	let json_array = json.as_array().unwrap();
	json_array.len()
}

#[tokio::main]
async fn main() -> Result<(), Error> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	let args = Args::parse();

	funded_accounts_json(args.n, args.ss58_prefix, &args.output, args.threads).await?;

	Ok(())
}
