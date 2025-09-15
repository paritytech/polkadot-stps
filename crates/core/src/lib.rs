mod logic;
mod models;

pub mod prelude {
    pub use crate::logic::*;
    pub use crate::models::*;

    // Polkadot/Substrate Crates
    pub use subxt::config::Config as SubxtConfig;
    pub use subxt::OnlineClient;

    // Third Party Crates
    pub use bon::Builder;
    pub use getset::Getters;
    pub use log::{debug, error, info, warn};
    pub use url::Url;
}
