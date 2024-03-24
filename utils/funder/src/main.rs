use clap::Parser;
use std::{error::Error, fs::File, path::PathBuf};
use subxt::ext::sp_core::{crypto::Ss58Codec, Pair};

const DEFAULT_FUNDED_JSON_PATH: &str = "funded-accounts.json";
const FUNDS: u64 = 10_000_000_000_000_000;
const DERIVATION: &str = "//Sender";

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
}

fn main() -> Result<(), Box<dyn Error>> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	let args = Args::parse();

	let accounts: Vec<_> = funder_lib::derive_accounts(args.n, DERIVATION.to_owned())
		.into_iter()
		.map(|p| (p.public().to_ss58check_with_version(args.ss58_prefix.into()), FUNDS))
		.collect();
	let mut file = File::create(args.output)?;
	serde_json::to_writer_pretty(&mut file, &serde_json::to_value(&accounts)?)?;

	Ok(())
}
