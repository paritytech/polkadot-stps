use crate::prelude::*;

pub enum TransactionBatchMode {
    SingleRecipient(Recipient),
    BatchOfRecipients(BatchOfRecipients),
}

pub type BatchOfRecipients = SetWithItemCountOfAtLeast<2, Recipient>;
