use clap::Parser;
use codec::Decode;
use futures::future::try_join_all;
use log::*;
use subxt::{
	extrinsic::Era, ClientBuilder, DefaultConfig, PairSigner, PolkadotExtrinsicParams,
	PolkadotExtrinsicParamsBuilder as Params,
	sp_core::{Pair, sr25519::Pair as SrPair}
};
use std::path::PathBuf;
use std::fs::File;
use std::io::{Write, Read};
use serde_json::Value;

#[derive(Parser)]
struct Cli {
	#[clap(subcommand)]
	command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
	/// Generate the JSON file to be used with Zombienet.
	FundAccountsJson(FundAccountsJsonArgs),
	/// Send many `Balance::transfer_keep_alive` to a node.
	SendBalanceTransfers(SendBalanceTransfersArgs),
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct FundAccountsJsonArgs {
	/// The number of accounts to fund
	#[clap(short, default_value_t = 500000)]
	n: usize,

	/// Path to write the funded accounts to.
	#[clap(long, short, default_value = "./funded-accounts.json")]
	output: PathBuf,

	/// Mnemonic blueprint to derive accounts with. An unique index will be appended.
	#[clap(long, short, default_value = "//Sender/")]
	mnemonic: String,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct SendBalanceTransfersArgs {
	/// The node to connect to.
	#[clap(long, short)]
	node: String,

	/// Number of extrinsics to send.
	///
	/// Defaults to the number of accounts in `funded_accounts`.
	/// Limited by the number of accounts in the `funded_accounts` file.
	#[clap(long, short)]
	extrinsics: Option<usize>,

	/// Chunk size for sending the extrinsics.
	#[clap(long, short, default_value_t = 50)]
	chunk_size: usize,

	/// Path to JSON file with the funded accounts.
	#[clap(long, short, default_value = "./funded-accounts.json")]
	funded_accounts: PathBuf,

	/// Mnemonic blueprint to derive accounts with. An unique index will be appended.
	#[clap(long, short, default_value = "//Sender/")]
	mnemonic: String,
}

#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod runtime {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	let cli = Cli::parse();
	match &cli.command {
		Commands::FundAccountsJson(args) => {
			let funded_accounts_json = funded_accounts_json(&args.mnemonic, args.n);
			let mut file = File::create(&args.output).unwrap();
			file.write_all(&funded_accounts_json).unwrap();
			info!("Wrote funded accounts to: {:?}", args.output);
		},
		Commands::SendBalanceTransfers(args) => {
			info!("Reading funded accounts from: {:?}", &args.funded_accounts);
			let mut file = File::open(&args.funded_accounts)?;
			let mut json_bytes = Vec::new();
			file.read_to_end(&mut json_bytes).expect("Unable to read data");

			let json: Value = serde_json::from_slice(&json_bytes)?;
			let json_array = json.as_array().unwrap();
			let n = args.extrinsics.unwrap_or(json_array.len());

			if n > json_array.len() {
				return Err(format!("Cannot send more extrinsics ({}) than accounts ({})", n, json_array.len()).into());
			}

			send_funds(&args.node, &args.mnemonic, args.chunk_size, n).await?;
		}
	}

	Ok(())
}

/// Initial funds for a genesis account.
const FUNDS: u64 = 10_000_000_000_000_000;

fn generate_signer(mnemonic_blueprint: &str, i: usize) -> PairSigner<DefaultConfig, SrPair> {
	let pair: SrPair = Pair::from_string(format!("{}{}", mnemonic_blueprint, i).as_str(), None).unwrap();
	let signer: PairSigner<DefaultConfig, SrPair> = PairSigner::new(pair);
	signer
}

fn funded_accounts_json(mnemonic_blueprint: &str, n: usize) -> Vec<u8> {
	let mut v = Vec::new();
	for i in 0..n {
		let signer = generate_signer(mnemonic_blueprint, i);
		let a: (String, u64) = (signer.account_id().to_string(), FUNDS);
		v.push(a);
	}

	let v_json = serde_json::to_value(&v).unwrap();
	serde_json::to_vec_pretty(&v_json).unwrap()
}

async fn send_funds(node: &String, mnemonic: &str, chunk_size: usize, n: usize) -> Result<(), Box<dyn std::error::Error>> {
	let receivers = generate_receivers(n); // one receiver per sender

	let api = ClientBuilder::new()
		.set_url(node)
		.build()
		.await?
		.to_runtime_api::<runtime::RuntimeApi<DefaultConfig, PolkadotExtrinsicParams<DefaultConfig>>>(
		);

	let ext_deposit = api.constants().balances().existential_deposit().unwrap();

	info!("Signing {} transactions", n);
	let mut txs = Vec::new();
	for i in 0..n {
		let signer = generate_signer(mnemonic, i);
		let tx_params = Params::new().era(Era::Immortal, *api.client.genesis());
		let tx = api
			.tx()
			.balances()
			.transfer_keep_alive(receivers[i as usize].clone().into(), ext_deposit)
			.create_signed(&signer, tx_params)
			.await?;
		txs.push(tx);
	}

	info!("Sending {} transactions in chunks of {}", n, chunk_size);
	let mut i = 0;
	let mut last_now = std::time::Instant::now();
	let mut last_sent = 0;
	let start = std::time::Instant::now();

	for chunk in txs.chunks(chunk_size) {
		let mut hashes = Vec::new();
		for tx in chunk {
			let hash = api.client.rpc().submit_extrinsic(tx);
			hashes.push(hash);
		}
		try_join_all(hashes).await?;

		let elapsed = last_now.elapsed();
		if elapsed >= std::time::Duration::from_secs(1) {
			let sent = i * chunk_size - last_sent;
			let rate = sent as f64 / elapsed.as_secs_f64();
			info!("{} txs sent in {} ms ({:.2} /s)", sent, elapsed.as_millis(), rate);
			last_now = std::time::Instant::now();
			last_sent = i * chunk_size;
		}
		i += 1;
	}
	let rate = n as f64 / start.elapsed().as_secs_f64();
	info!("{} txs sent in {} ms ({:.2} /s)", n, start.elapsed().as_millis(), rate);

	Ok(())
}

/// Generates a vector of account IDs.
fn generate_receivers(num: usize) -> Vec<subxt::sp_core::crypto::AccountId32> {
	let mut receivers = Vec::new();
	for i in 0..num {
		// Decode the account ID from the string:
		let account_id = Decode::decode(&mut &format!("{:0>32?}", i).as_bytes()[..])
			.expect("Must decode account ID");
		receivers.push(account_id);
	}
	debug!("Generated {} receiver addresses", receivers.len());
	receivers
}

#[cfg(test)]
mod tests {
	use std::collections::BTreeSet as Set;

	#[test]
	/// Check that the generated addresses are unique.
	fn generate_receivers_unique() {
		let receivers = super::generate_receivers(1024);
		let set: Set<_> = receivers.iter().collect();

		assert_eq!(set.len(), receivers.len());
	}
}
