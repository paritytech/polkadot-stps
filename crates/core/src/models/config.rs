use crate::prelude::*;

#[derive(Debug, Clone, Getters, Builder)]
pub struct Config {
    #[getset(get = "pub")]
    node_url: Url,

    #[getset(get = "pub")]
    number_of_sending_accounts: usize,

    #[getset(get = "pub")]
    tps: usize,
}
