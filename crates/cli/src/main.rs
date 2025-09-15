mod logic;
mod models;

mod prelude {
    pub use crate::logic::*;
    pub use crate::models::*;

    pub use polkadot_tx_spammer_core::prelude::*;
}

#[tokio::main]
async fn main() {
    use clap::Parser as _;
    use prelude::*;

    init_logging();
    let cli_args = CliArgs::parse();
    run(cli_args).await
}
