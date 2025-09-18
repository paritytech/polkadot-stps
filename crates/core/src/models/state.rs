use getset::MutGetters;

use crate::prelude::*;

pub type Recipient = AnyAccountId;

#[derive(Debug, Clone, Getters, MutGetters, Builder)]
pub struct State {
    #[getset(get = "pub")]
    chain: Chain,

    #[getset(get = "pub")]
    api: Api,

    #[getset(get_mut = "pub")]
    senders: IndexSet<Sender>,

    #[getset(get = "pub")]
    recipients: IndexSet<Recipient>,

    #[getset(get = "pub")]
    tps: usize,
}
