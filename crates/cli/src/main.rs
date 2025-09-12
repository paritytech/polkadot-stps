mod logic;
mod models;

mod prelude {
    pub use crate::models::*;
    pub use crate::logic::*;

    pub use polkadot_tx_spammer_core::prelude::*;
}

fn main() {
    use prelude::*;
    use clap::Parser as _;
    
    init_logging();
    let cli_args = CliArgs::parse();
    run(cli_args);

}