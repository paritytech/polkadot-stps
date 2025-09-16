
use subxt::ext::codec::Decode;
use subxt::storage::Storage;

use crate::prelude::*;

type AccountInfo = frame_system::AccountInfo<u32, pallet_balances::AccountData<u128>>;

pub type Nonce = u64;

#[derive(Debug, thiserror::Error)]
pub enum GetNonceError {
    #[error("Failed to get account storage")]
    AccountStorageFetchFailed { underlying: subxt::Error },

    #[error("Failed to get account status")]
    AccountStatusFetchFailed { underlying: String },

    #[error("Failed to decode encoded account state")]
    EncodedAccountStateDecodeFailed { underlying: String },

    #[error("Account nonce not set but it should be")]
    AccountNonceNotSet,
}

async fn get_account_storage<C>(
    api: &OnlineClient<C>,
) -> Result<Storage<C, OnlineClient<C>>, GetNonceError>
where
    C: SubxtConfig,
{
    api.storage()
        .at_latest()
        .await
        .map_err(|e| GetNonceError::AccountStorageFetchFailed { underlying: e })
}

pub async fn get_encoded_account_state<C>(
    api: &OnlineClient<C>,
    account_id: impl Into<subxt::dynamic::Value>,
) -> Result<Option<Vec<u8>>, GetNonceError>
where
    C: SubxtConfig,
{
    let account_id = account_id.into();
    let account_storage = get_account_storage(api).await?;
    let account_state_storage_addr = subxt::dynamic::storage("System", "Account", vec![account_id]);
    let account_state_encoded = account_storage
        .fetch(&account_state_storage_addr)
        .await
        .map_err(|e| GetNonceError::AccountStatusFetchFailed {
            underlying: e.to_debug_string(),
        })?
        .map(|v| v.into_encoded());
    Ok(account_state_encoded)
}

impl From<AnyAccountId> for subxt::dynamic::Value {
    fn from(account_id: AnyAccountId) -> Self {
        match account_id {
            AnyAccountId::EthereumCompat(a) => subxt::dynamic::Value::from_bytes(a.0),
            AnyAccountId::PolkadotBased(a) => {
                subxt::dynamic::Value::from_bytes(a.as_ref().as_ref() as &[u8])
            }
        }
    }
}

/// Fetch the current nonce for an account
pub async fn get_nonce<C>(
    api: &OnlineClient<C>,
    account_id: impl Into<subxt::dynamic::Value>,
) -> Result<Nonce, GetNonceError>
where
    C: SubxtConfig,
{
    let Some(encoded_account_state) = get_encoded_account_state(api, account_id).await? else {
        let default_nonce = Nonce::default();
        info!("Using default nonce {default_nonce}");
        return Ok(default_nonce);
    };
    let account_state: AccountInfo =
        Decode::decode(&mut &encoded_account_state[..]).map_err(|e| {
            GetNonceError::EncodedAccountStateDecodeFailed {
                underlying: e.to_debug_string(),
            }
        })?;
    let nonce = account_state.nonce as Nonce;
    info!("Fetched nonce: {nonce}");
    Ok(nonce)
}
