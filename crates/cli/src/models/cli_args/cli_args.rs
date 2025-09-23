use crate::prelude::*;
use clap::{Parser, Subcommand};

pub const BINARY_NAME: &str = "spammer";
pub const DEFAULT_SENDER_SEED: &str = "//Sender";
pub const DEFAULT_RECEIVER_SEED: &str = "//Receiver";

#[derive(Debug, Parser)]
#[command(name = BINARY_NAME, about = "Generate invoices for services and expenses, with support for emailing them.")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct CliArgs {
    /// The command to run, either for generating an invoice or for data management.
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Spammer(SpammerArgs),
}
