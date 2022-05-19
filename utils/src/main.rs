use clap::Parser;
use log::*;
use serde_json::Value;
use std::{
	fs::File,
	io::{Read, Write},
	path::PathBuf,
};

mod funder;
mod pre;
mod sender;
mod tps;

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
	/// Check pre-conditions (account nonce and free balance).
	CheckPreConditions(CheckPreConditionsArgs),
	/// Calculate TPS on finalized blocks
	CalculateTPS(CalculateTPSArgs),
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

	/// Derivation blueprint to derive accounts with. An unique index will be appended.
	#[clap(long, short, default_value = "//Sender/")]
	derivation: String,
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

	/// derivation blueprint to derive accounts with. An unique index will be appended.
	#[clap(long, short, default_value = "//Sender/")]
	derivation: String,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CheckPreConditionsArgs {
	/// The node to connect to.
	#[clap(long)]
	node: String,

	/// derivation blueprint to derive accounts with. An unique index will be appended.
	#[clap(long, short, default_value = "//Sender/")]
	derivation: String,

	/// The number of prefunded accounts
	#[clap(short, default_value_t = 500000)]
	n: usize,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CalculateTPSArgs {
	/// The node to connect to.
	#[clap(long)]
	node: String,

	/// The number of sent transactions
	#[clap(short)]
	n: usize,
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
			let funded_accounts_json = funder::funded_accounts_json(&args.derivation, args.n);
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
				return Err(format!(
					"Cannot send more extrinsics ({}) than accounts ({})",
					n,
					json_array.len()
				)
				.into())
			}

			sender::send_funds(&args.node, &args.derivation, args.chunk_size, n).await?;
		},
		Commands::CheckPreConditions(args) => {
			info!("Checking sTPS pre-conditions (account nonces and free balances).");
			pre::pre_conditions(&args.node, &args.derivation, args.n).await?;
		},
		Commands::CalculateTPS(args) => {
			info!("Calculating TPS on finalized blocks.");
			tps::calc_tps(&args.node, args.n).await?;
		},
	}

	Ok(())
}
