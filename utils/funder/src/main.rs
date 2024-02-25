use clap::Parser;
use std::path::PathBuf;
use utils::Error;

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

#[tokio::main]
async fn main() -> Result<(), Error> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	let args = Args::parse();

	funder_lib::funded_accounts_json(args.n, args.ss58_prefix, &args.output, args.threads).await?;

	Ok(())
}
