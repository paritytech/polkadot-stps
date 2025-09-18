use crate::prelude::*;

use futures::future::try_join_all;
use tokio::task;

const SLEEP_BETWEEN_TICK_DURATION_MS: u64 = 1000;

#[bon]
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
        let (api, recipients) = {
            let s = self.state();
            (s.api().clone(), s.recipients().clone())
        };

        let senders: Vec<_> = { self.state_mut().senders_mut().iter().cloned().collect() };

        let handles: Vec<_> = senders
            .into_iter()
            .map(|sender| {
                let api = api.clone();
                let recipients = recipients.clone();
                task::spawn(async move {
                    Self::submit_transactions()
                        .to(recipients)
                        .from(sender)
                        .using(&api)
                        .call()
                        .await
                })
            })
            .collect();

        try_join_all(handles)
            .await
            .map_err(|e| Error::JoinSendersError(Box::new(e)))?;
        Ok(())
    }

    #[builder]
    async fn submit_transactions(
        to: IndexSet<Recipient>,
        from: Sender,
        using: &Api,
    ) -> Result<(), Error> {
        let (recipients, sender, api) = (to, from, using);
        sender
            .submit_transactions()
            .to(recipients)
            .using(api)
            .call()
            .await
    }
}
