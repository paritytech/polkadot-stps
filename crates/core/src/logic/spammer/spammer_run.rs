
use crate::prelude::*;

use futures::future::try_join_all;
use tokio::{
    task,
    time::{sleep, Duration},
};

impl Spammer {
    pub async fn run(&mut self) -> Result<(), Error> {
        let handles: Vec<_> = self
            .state_mut()
            .senders()
            .iter()
            .enumerate()
            .map(|(index_of_sending_account, _sender)| {
                task::spawn(async move {
                    sleep(Duration::from_millis(index_of_sending_account as u64 * 20)).await;
                    println!("Task {index_of_sending_account} done");
                })
            })
            .collect();

        // Wait for all tasks to finish (panic on join error like before)
        try_join_all(handles).await.unwrap();
        Ok(())
    }

    fn api(&self) -> &Api {
        self.state().api()
    }

    async fn submit_transaction(
        &self,
        sender: &mut Sender,
        recipients: Vec<AnyKeyPair>,
    ) -> Result<(), Error> {
        sender.submit_transaction(self.api(), recipients)
    }
}
