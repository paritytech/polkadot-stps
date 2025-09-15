use crate::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Sender {
    nonce: Nonce,
    signer: AnySigner,
}

impl Sender {
    pub(crate) async fn new(
        key_pair: AnyKeyPair,
        api: &OnlineClient<AnyConfig>,
    ) -> Result<Self, Error> {
        use subxt::tx::Signer;
        let signer = AnySigner::from(key_pair);
        let nonce = get_nonce(api, signer.account_id()).await?;
        Ok(Self { nonce, signer })
    }

    pub(crate) fn submit_transaction(
        &mut self,
        api: &OnlineClient<AnyConfig>,
        recipients: impl IntoIterator<Item = AnyKeyPair>,
    ) -> Result<(), Error> {
        let recipients = recipients.into_iter().collect::<IndexSet<AnyKeyPair>>();

        // recipients.into_iter()
        // let individual_txs = subxt::dynamic::tx(
        //     "Balances",
        //     "transfer_keep_alive",
        // 		vec![inputs.receiver_id_values[0].clone(), Value::u128(TX_TRANSFER_AMOUNT)],
        // )
        // .into_value();

        // let batched_tx = subxt::dynamic::tx(
        //     "Utility",
        //     "batch",
        //     vec![TxValue::named_composite(vec![(
        //         "calls",
        //         individual_txs.into(),
        //     )])],
        // );

        // let tx_params = DefaultExtrinsicParamsBuilder::new()
        //     .nonce(self.nonce)
        //     .build();

        // self.nonce += 1;

        // api.tx()
        //     .create_partial_offline(&batched_tx, tx_params)
        //     .unwrap()
        //     .sign(&self.signer)

        todo!()
    }
}
