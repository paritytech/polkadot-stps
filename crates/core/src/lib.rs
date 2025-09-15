mod logic;
mod models;

pub mod prelude {
    pub use crate::logic::*;
    pub use crate::models::*;

    pub use bon::Builder;
    pub use getset::Getters;
    pub use log::{debug, error, info, warn};
    pub use url::Url;
}
