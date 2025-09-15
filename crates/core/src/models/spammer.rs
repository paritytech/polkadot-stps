use getset::MutGetters;

use crate::prelude::*;

#[derive(Debug, Getters, MutGetters, Builder)]
pub struct Spammer {
    #[getset(get = "pub", get_mut = "pub")]
    state: State,
}
