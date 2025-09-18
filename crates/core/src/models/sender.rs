
use crate::prelude::*;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use subxt::config::polkadot::PolkadotExtrinsicParamsBuilder as GenTxParams;

pub type TxParams = GenTxParams<AnyConfig>;

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

impl Sender {
    pub(crate) async fn new(
        signer: AnySigner,
        api: &OnlineClient<AnyConfig>,
    ) -> Result<Self, Error> {
        use subxt::tx::Signer;
        let nonce = get_nonce(api, signer.account_id())
            .await
            .map_err(|e| Error::GetNonceError(Box::new(e)))?;
        Ok(Self {
            nonce: Arc::new(AtomicU64::new(nonce)),
            signer,
        })
    }

    async fn submit_transaction(
        &self,
        api: &OnlineClient<AnyConfig>,
        transaction: Transaction,
    ) -> Result<(), Error> {
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

    pub(crate) async fn submit_transactions(
        &self,
        api: &OnlineClient<AnyConfig>,
        recipients: IndexSet<Receiver>,
    ) -> Result<(), Error> {
        let nonce = self.next_nonce();
        let transaction = Transaction::transfer()
            .recipients(recipients)
            .nonce(nonce)
            .call();
        self.submit_transaction(api, transaction).await
    }
}


pub type Recipient = AnyAccountId;
