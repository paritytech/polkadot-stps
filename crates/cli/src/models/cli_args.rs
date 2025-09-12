use crate::prelude::*;
use clap::Parser;

pub const BINARY_NAME: &str = "spammer";

/// CLI utility to generate transaction load against a Substrate-based node.
///
/// Spawns multiple sender workers that continuously submit balance transfers
/// (optionally in batches), monitors best blocks to compute TPS, and throttles
/// submissions based on the backlog of un-included transactions. Use `--seed`
/// to pre-fund derived sender accounts from `//Alice` before starting.
#[derive(Parser, Debug)]
#[command(name = BINARY_NAME, author, version, about, long_about = None)]
pub struct CliArgs {
    /// Node URL. Can be either a collator, or relaychain node based on whether you want to measure parachain TPS, or relaychain TPS.
    #[arg(long)]
    node_url: String,

    /// Total number of senders
    #[arg(long)]
    total_senders: Option<usize>,

    /// Target Transactions Per Second ("TPS") to maintain, if `total_senders` is
    /// set and is less than `tps`, then `total_senders` will be used as the TPS.
    /// If set and greater than `tps` then some senders will be idle. If `total_senders`
    /// is not set then we will set it to `tps` (one transaction per sender per second).
    #[arg(long, default_value_t = 10)]
    tps: usize,
}

impl TryFrom<CliArgs> for Config {
    type Error = InvalidCliArgs;

    fn try_from(cli_args: CliArgs) -> Result<Self, Self::Error> {
        let Ok(url) = Url::parse(&cli_args.node_url) else {
            return Err(InvalidCliArgs::NodeUrlInvalid {
                bad_value: cli_args.node_url.clone(),
            });
        };
        let number_of_sending_accounts = cli_args.total_senders.unwrap_or(cli_args.tps);
        if number_of_sending_accounts == 0 {
            return Err(InvalidCliArgs::TotalSendersMustBePositive);
        }
        if cli_args.tps == 0 {
            return Err(InvalidCliArgs::TpsCannotBeZero);
        }
        Ok(Config::builder()
            .node_url(url)
            .number_of_sending_accounts(number_of_sending_accounts)
            .tps(cli_args.tps)
            .build())
    }
}
