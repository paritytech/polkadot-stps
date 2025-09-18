use crate::prelude::*;

use subxt::tx::{DynamicPayload, PartialTransaction};

// Later we might change this into a tagged union if we want to support more transaction types.
pub type Transaction = TokenTransferTransaction;

#[bon]
impl Transaction {
    #[builder]
    pub fn transfer(recipients: IndexSet<Recipient>, nonce: Nonce, amount: Option<u128>) -> Self {
        let recipients = SetWithItemCountOfAtLeast::new(recipients);
        TokenTransferTransaction::builder()
            .recipients(recipients)
            .maybe_amount(amount)
            .nonce(nonce)
            .build()
    }

    fn tx_payload_single_recipient(&self, receiver: AnyAccountId) -> DynamicPayload {
        subxt::dynamic::tx(
            "Balances",
            "transfer_keep_alive",
            vec![
                subxt::dynamic::Value::unnamed_variant(
                    "Id",
                    [subxt::dynamic::Value::from(receiver)],
                ),
                subxt::dynamic::Value::u128(self.amount().unwrap_or(1u32.into())),
            ],
        )
    }

    fn tx_payload_batch_of_recipients(
        &self,
        batch_of_recipients: BatchOfRecipients,
    ) -> DynamicPayload {
        let calls: Vec<subxt::dynamic::Value> = batch_of_recipients
            .as_ref()
            .clone()
            .into_iter()
            .map(|r| self.tx_payload_single_recipient(r).into_value())
            .collect();
        subxt::dynamic::tx(
            "Utility",
            "batch_all",
            vec![subxt::dynamic::Value::from(calls)],
        )
    }

    fn batch_mode(&self) -> TransactionBatchMode {
        let recipients = self.recipients().clone();
        if recipients.len() == 1 {
            TransactionBatchMode::SingleRecipient(recipients.iter().next().unwrap().clone())
        } else {
            TransactionBatchMode::BatchOfRecipients(SetWithItemCountOfAtLeast::<2, _>::new(
                recipients,
            ))
        }
    }

    fn into_tx_payload(self) -> DynamicPayload {
        match self.batch_mode() {
            TransactionBatchMode::SingleRecipient(r) => self.tx_payload_single_recipient(r),
            TransactionBatchMode::BatchOfRecipients(rs) => self.tx_payload_batch_of_recipients(rs),
        }
    }

    pub fn into_signable_tx(self, api: &Api) -> Result<SignableTx> {
        let tx_params = TxParams::new().nonce(*self.nonce()).build();
        let tx_payload = self.into_tx_payload();
        let tx: SignableTx = api
            .tx()
            .create_partial_offline(&tx_payload, tx_params)
            .expect("Failed to create partial offline transaction"); // TODO change to map_err
        Ok(tx)
    }
}

pub type SignableTx = PartialTransaction<AnyConfig, Api>;

#[derive(Debug, Clone, Builder, Getters, PartialEq, Eq, Hash, derive_more::Display)]
#[display("Transfer {{ recipients: {}, amount: {}, nonce: {} }}", recipients.len(), amount.unwrap_or_default(), nonce)]
pub struct TokenTransferTransaction {
    #[getset(get = "pub")]
    recipients: SetWithItemCountOfAtLeast<1, Recipient>,

    /// If None is specified, then `1` will be used.
    #[getset(get = "pub")]
    amount: Option<u128>,

    #[getset(get = "pub")]
    nonce: Nonce,
}
