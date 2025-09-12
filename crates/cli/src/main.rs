mod run;

mod prelude {
    pub use crate::run::*;

    pub use polkadot_tx_spammer_core::prelude::*;
}

fn main() {
    use prelude::*;
    let _config = Config;
    run();

}