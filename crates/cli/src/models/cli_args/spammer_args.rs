use crate::prelude::*;
use clap::Parser;


#[derive(Parser, Debug)]
pub struct SpammerArgs {
    /// Node URL. Can be either a collator, or relaychain node based on whether you want to measure parachain TPS, or relaychain TPS.
    #[arg(long)]
    node_url: String,

    /// Total number of senders
    #[arg(long)]
    total_senders: Option<usize>,

    #[arg(long, default_value_t = DEFAULT_SENDER_SEED.to_owned())]
    sender_seed: String,

    #[arg(long, default_value_t = DEFAULT_RECEIVER_SEED.to_owned())]
    receiver_seed: String,

    /// Target Transactions Per Second ("TPS") to maintain, if `total_senders` is
    /// set and is less than `tps`, then `total_senders` will be used as the TPS.
    /// If set and greater than `tps` then some senders will be idle. If `total_senders`
    /// is not set then we will set it to `tps` (one transaction per sender per second).
    #[arg(long, default_value_t = 10)]
    tps: usize,

    /// Use Ethereum-compatible transactions (default: `false` => Polkadot-based)
    #[arg(long, default_value_t = false)]
    eth: bool,
}

impl TryFrom<SpammerArgs> for SpammerParameters {
    type Error = InvalidCliArgs;

    fn try_from(args: SpammerArgs) -> Result<Self, Self::Error> {
        let Ok(url) = Url::parse(&args.node_url) else {
            return Err(InvalidCliArgs::NodeUrlInvalid {
                bad_value: args.node_url.clone(),
            });
        };
        let number_of_sending_accounts = args.total_senders.unwrap_or(args.tps);
        if number_of_sending_accounts == 0 {
            return Err(InvalidCliArgs::TotalSendersMustBePositive);
        }
        if args.tps == 0 {
            return Err(InvalidCliArgs::TpsCannotBeZero);
        }

        let chain = if args.eth {
            Chain::Ethereum
        } else {
            Chain::PolkadotBased
        };

        Ok(SpammerParameters::builder()
            .node_url(url)
            .number_of_sending_accounts(number_of_sending_accounts)
            .tps(args.tps)
            .chain(chain)
            .sender_seed(args.sender_seed.clone())
            .receiver_seed(args.receiver_seed.clone())
            .build())
    }
}
