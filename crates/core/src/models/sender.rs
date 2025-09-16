use crate::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Sender {
    nonce: Nonce,
    signer: AnySigner,
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
        Ok(Self { nonce, signer })
    }

    pub(crate) async fn from_pair(
        key_pair: AnyKeyPair,
        api: &OnlineClient<AnyConfig>,
    ) -> Result<Self, Error> {
        let signer = AnySigner::from(key_pair);
        Self::new(signer, api).await
    }

    pub(crate) fn submit_transaction(
        &mut self,
        _api: &OnlineClient<AnyConfig>,
        _recipients: IndexSet<Receiver>,
    ) -> Result<(), Error> {
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

        self.nonce += 1;

        // api.tx()
        //     .create_partial_offline(&batched_tx, tx_params)
        //     .unwrap()
        //     .sign(&self.signer)

        todo!()
    }
}
