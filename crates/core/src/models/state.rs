use crate::prelude::*;


#[derive(Debug, Clone, Getters, Builder)]
pub struct State {
    #[getset(get = "pub")]
    chain: Chain,

    #[getset(get = "pub")]
    api: Api,

    #[getset(get = "pub")]
    senders: IndexSet<Sender>,
    
    #[getset(get = "pub")]
    receivers: IndexSet<AnyAccountId>,

    #[getset(get = "pub")]
    tps: usize,
}