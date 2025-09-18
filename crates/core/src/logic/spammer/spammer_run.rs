use crate::prelude::*;

use futures::future::try_join_all;
use tokio::task;

const SLEEP_BETWEEN_TICK_DURATION_MS: u64 = 1000;

impl Spammer {
    pub async fn run(&mut self) -> Result<(), Error> {
        loop {
            self.tick().await?;
            Self::sleep_between_ticks().await;
        }
    }

    async fn sleep_between_ticks() {
        log::info!(
            "Sleeping between ticks ({} ms)",
            SLEEP_BETWEEN_TICK_DURATION_MS
        );
        tokio::time::sleep(std::time::Duration::from_millis(
            SLEEP_BETWEEN_TICK_DURATION_MS,
        ))
        .await;
    }

    async fn tick(&mut self) -> Result<(), Error> {
        let (api, receivers) = {
            let s = self.state();
            (s.api().clone(), s.receivers().clone())
        };

        let senders: Vec<_> = { self.state_mut().senders_mut().iter().cloned().collect() };

        let handles: Vec<_> = senders
            .into_iter()
            .map(|sender| {
                let api = api.clone();
                let receivers = receivers.clone();
                task::spawn(
                    async move { Self::submit_transactions_for(api, receivers, sender).await },
                )
            })
            .collect();

        try_join_all(handles)
            .await
            .map_err(|e| Error::JoinSendersError(Box::new(e)))?;
        Ok(())
    }

    async fn submit_transactions_for(
        api: Api,
        receivers: IndexSet<Receiver>,
        sender: Sender,
    ) -> Result<(), Error> {
        sender.submit_transactions(&api, receivers).await
    }
}
