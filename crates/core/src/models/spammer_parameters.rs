use crate::prelude::*;

#[derive(Debug, Clone, Getters, Builder)]
pub struct SpammerParameters {
    #[getset(get = "pub")]
    node_url: Url,

    #[getset(get = "pub")]
    number_of_sending_accounts: usize,

    #[getset(get = "pub")]
    sender_seed: String,

    #[getset(get = "pub")]
    receiver_seed: String,

    #[getset(get = "pub")]
    tps: usize,

    #[getset(get = "pub")]
    chain: Chain,
}

pub type Api = OnlineClient<AnyConfig>;
