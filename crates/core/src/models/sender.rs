use crate::prelude::*;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

#[derive(Debug, Clone)]
pub(crate) struct Sender {
    // Use interior mutability so we can mutate via &self inside async tasks.
    nonce: Arc<AtomicU64>,
    signer: AnySigner,
}

// Equality and hashing are based solely on the signer identity.
impl PartialEq for Sender {
    fn eq(&self, other: &Self) -> bool {
        self.signer == other.signer
    }
}
impl Eq for Sender {}
impl std::hash::Hash for Sender {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.signer.hash(state)
    }
}

#[bon]
impl Sender {
    pub(crate) async fn new(
        signer: AnySigner,
        api: &OnlineClient<AnyConfig>,
    ) -> Result<Self, Error> {
        let nonce = get_nonce()
            .of(&signer)
            .using(api)
            .call()
            .await
            .map_err(|e| Error::GetNonceError(Box::new(e)))?;

        Ok(Self {
            nonce: Arc::new(AtomicU64::new(nonce)),
            signer,
        })
    }

    #[builder]
    async fn submit(
        &self,
        transaction: Transaction,
        using: &OnlineClient<AnyConfig>,
    ) -> Result<(), Error> {
        let api = using;
        let tx_description = transaction.to_string();
        let mut signable = transaction.into_signable_tx(api)?;
        let submittable = signable // TODO change to map_err
            .sign(&self.signer);

        info!("Submitting transaction: {}", tx_description);
        submittable.submit_and_watch().await.expect("should work"); // TODO change to map_err
        info!("Transaction submitted");
        Ok(())
    }

    fn next_nonce(&self) -> Nonce {
        self.nonce.fetch_add(1, Ordering::SeqCst)
    }

    #[builder]
    pub(crate) async fn submit_transactions(
        &self,
        to: IndexSet<Recipient>,
        using: &OnlineClient<AnyConfig>,
    ) -> Result<(), Error> {
        let (recipients, api) = (to, using);
        let nonce = self.next_nonce();
        let transaction = Transaction::transfer()
            .recipients(recipients)
            .nonce(nonce)
            .call();
        self.submit()
            .transaction(transaction)
            .using(api)
            .call()
            .await
    }
}
