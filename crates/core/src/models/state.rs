use crate::prelude::*;

pub type Receiver = AnyAccountId;

#[derive(Debug, Clone, Getters, Builder)]
pub struct State {
    #[getset(get = "pub")]
    chain: Chain,

    #[getset(get = "pub")]
    api: Api,

    #[getset(get = "pub")]
    senders: IndexSet<Sender>,

    #[getset(get = "pub")]
    receivers: IndexSet<Receiver>,

    #[getset(get = "pub")]
    tps: usize,
}
