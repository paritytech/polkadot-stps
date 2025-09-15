use crate::prelude::*;

use futures::future::try_join_all;
use tokio::{
    task,
    time::{sleep, Duration},
};

#[derive(Debug, Builder)]
pub struct Spammer {
    #[builder(into)]
    config: Config,
}

impl Spammer {
    pub async fn run(&self) -> Result<(), Error> {
        let number_of_sending_accounts = *self.config.number_of_sending_accounts();

        let handles: Vec<_> = (0..number_of_sending_accounts)
            .map(|index_of_sending_account| {
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
}
