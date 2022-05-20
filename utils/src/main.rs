use clap::Parser;
use log::*;
use std::path::PathBuf;

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

const DEFAULT_FUNDED_JSON_PATH: &str = "tests/stps/funded-accounts.json";
const DEFAULT_DERIVATION: &str = "//Sender/";

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct FundAccountsJsonArgs {
	/// The number of accounts to fund
	#[clap(short, default_value_t = 500000)]
	n: usize,

	/// Path to write the funded accounts to.
	#[clap(long, short, default_value = DEFAULT_FUNDED_JSON_PATH)]
	output: PathBuf,

	/// Derivation blueprint to derive accounts with. An unique index will be appended.
	#[clap(long, short, default_value = DEFAULT_DERIVATION)]
	derivation: String,

	/// Number of threads to derive accounts with.
	#[clap(long, short, default_value = "4")]
	threads: usize,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct SendBalanceTransfersArgs {
	/// The node to connect to.
	#[clap(long, short)]
	node: String,

	/// Chunk size for sending the extrinsics.
	#[clap(long, short, default_value_t = 50)]
	chunk_size: usize,

	/// Path to JSON file with the funded accounts.
	#[clap(long, short, default_value = DEFAULT_FUNDED_JSON_PATH)]
	funded_accounts: PathBuf,

	/// derivation blueprint to derive accounts with. An unique index will be appended.
	#[clap(long, short, default_value = DEFAULT_DERIVATION)]
	derivation: String,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CheckPreConditionsArgs {
	/// The node to connect to.
	#[clap(long)]
	node: String,

	/// derivation blueprint to derive accounts with. An unique index will be appended.
	#[clap(long, short, default_value = DEFAULT_DERIVATION)]
	derivation: String,

	/// Path to JSON file with the funded accounts.
	#[clap(long, short, default_value = DEFAULT_FUNDED_JSON_PATH)]
	funded_accounts: PathBuf,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CalculateTPSArgs {
	/// The node to connect to.
	#[clap(long)]
	node: String,

	/// Path to JSON file with the funded accounts.
	#[clap(long, short, default_value = DEFAULT_FUNDED_JSON_PATH)]
	funded_accounts: PathBuf,
}

#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod runtime {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	let cli = Cli::parse();
	match cli.command {
		Commands::FundAccountsJson(args) => {
			funder::funded_accounts_json(&args.derivation, args.n, &args.output, args.threads)
				.await?;
			info!("Wrote funded accounts to: {:?}", args.output);
		},
		Commands::SendBalanceTransfers(args) => {
			info!("Reading funded accounts from: {:?}", &args.funded_accounts);
			let n = funder::n_accounts(&args.funded_accounts);
			sender::send_funds(args.node, &args.derivation, args.chunk_size, n).await?;
		},
		Commands::CheckPreConditions(args) => {
			info!("Checking sTPS pre-conditions (account nonces and free balances).");
			let n = funder::n_accounts(&args.funded_accounts);
			pre::pre_conditions(&args.node, &args.derivation, n).await?;
		},
		Commands::CalculateTPS(args) => {
			info!("Calculating TPS on finalized blocks.");
			let n = funder::n_accounts(&args.funded_accounts);
			tps::calc_tps(&args.node, n).await?;
		},
	}

	Ok(())
}
