use std::sync::Arc;

use crate::prelude::*;

use futures::future::join_all;
use indexmap::IndexSet;
use jsonrpsee_core::client::Client;
use subxt::{
    backend::legacy::LegacyBackend,
    ext::jsonrpsee::client_transport::ws::WsTransportClientBuilder,
};
use tokio::time::Duration;

async fn create_api(node_url: Url) -> Result<Api, BootstrapSpammerError> {
    let (node_sender, node_receiver) = WsTransportClientBuilder::default()
        .build(node_url)
        .await
        .map_err(|e| BootstrapSpammerError::CreateApiFailure {
            underlying: e.to_debug_string(),
        })?;

    let client = Client::builder()
        .request_timeout(Duration::from_secs(3600))
        .max_buffer_capacity_per_subscription(4096 * 1024)
        .max_concurrent_requests(2 * 1024 * 1024)
        .build_with_tokio(node_sender, node_receiver);

    let backend = LegacyBackend::builder().build(client);
    let api = Api::from_backend(Arc::new(backend)).await.map_err(|e| {
        BootstrapSpammerError::CreateApiFailure {
            underlying: e.to_debug_string(),
        }
    })?;

    Ok(api)
}

impl Spammer {
    pub async fn bootstrap(parameters: Parameters) -> Result<Self, Error> {
        let api = create_api(parameters.node_url().clone())
            .await
            .map_err(Error::Bootstrap)?;

        let number_of_sending_accounts = *parameters.number_of_sending_accounts();
        let number_of_receiving_accounts = 10;

        let sender_key_pairs = derive_accounts(
            number_of_sending_accounts,
            parameters.sender_seed().clone(),
            *parameters.chain(),
        );

        let receiver_key_pairs = derive_accounts(
            number_of_receiving_accounts,
            parameters.receiver_seed().clone(),
            *parameters.chain(),
        );
        let receivers = receiver_key_pairs
            .into_iter()
            .map(AnyAccountId::from)
            .collect::<IndexSet<_>>();

        let senders = sender_key_pairs
            .into_iter()
            .map(|signer| Sender::new(signer, &api))
            .collect::<Vec<_>>();

        let senders = join_all(senders)
            .await
            .into_iter()
            .collect::<Result<IndexSet<_>, _>>()?;

        let state = State::builder()
            .chain(*parameters.chain())
            .api(api)
            .senders(senders)
            .receivers(receivers)
            .tps(*parameters.tps())
            .build();

        Ok(Self::builder().state(state).build())
    }
}
