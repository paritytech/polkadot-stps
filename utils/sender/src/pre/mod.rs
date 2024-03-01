use codec::Decode;
use log::*;
use subxt::client::OfflineClientT;
use subxt::ext::sp_core::{sr25519::Pair as SrPair, Pair};
use subxt::{tx::PairSigner, utils::AccountId32, PolkadotConfig};

use utils::{Api, Error, DERIVATION};

/// Check pre-conditions of accounts attributed to this sender
pub async fn pre_conditions(api: &Api, i: &usize, n: &usize) -> Result<(), Error> {
	info!(
		"Sender {}: checking pre-conditions of accounts {}{} through {}{}",
		i,
		DERIVATION,
		i * n,
		DERIVATION,
		(i + 1) * n - 1
	);
	for j in i * n..(i + 1) * n {
		let pair: SrPair =
			Pair::from_string(format!("{}{}", DERIVATION, j).as_str(), None).unwrap();
		let signer: PairSigner<PolkadotConfig, SrPair> = PairSigner::new(pair);
		let account = signer.account_id();
		debug!("Sender {}: checking account {}", i, account);
		check_account(&api, account).await?;
	}
	debug!("Sender {}: all pre-conditions checked and succeeded!", i);
	Ok(())
}

/// Use JoinSet to run prechecks in a multi-threaded way.
/// The pre_condition call is async because it fetches the chain state and hence is I/O bound.
pub async fn parallel_pre_conditions(
	api: &Api,
	threads: usize,
	n_tx_sender: usize,
) -> Result<(), Error> {
	let mut precheck_set = tokio::task::JoinSet::new();
	for i in 0..threads {
		let api = api.clone();
		let n_tx_sender = n_tx_sender.clone();
		precheck_set.spawn(async move {
			match pre_conditions(&api, &i, &n_tx_sender).await {
				Ok(_) => Ok(()),
				Err(e) => Err(e),
			}
		});
	}
	while let Some(result) = precheck_set.join_next().await {
		match result {
			Ok(_) => (),
			Err(e) => {
				error!("Error: {:?}", e);
			},
		}
	}
	Ok(())
}

// FIXME: This assumes that all the chains supported by sTPS use this `AccountInfo` type. Currently,
// that holds. However, to benchmark a chain with another `AccountInfo` structure, a mechanism to
// adjust this type info should be provided.
type AccountInfo = frame_system::AccountInfo<u32, pallet_balances::AccountData<u128>>;

/// Check account nonce and free balance
async fn check_account(api: &Api, account: &AccountId32) -> Result<(), Error> {
	let ext_deposit_query = subxt::dynamic::constant("Balances", "ExistentialDeposit");
	let ext_deposit = api
		.constants()
		.at(&ext_deposit_query)?
		.to_value()?
		.as_u128()
		.expect("Only u128 deposits are supported");
	let account_state_storage_addr = subxt::dynamic::storage("System", "Account", vec![account]);
	let finalized_head_hash = api.backend().latest_finalized_block_ref().await?.hash();
	// let finalized_head_hash = api.rpc().finalized_head().await?;
	let account_state_encoded = api
		.storage()
		.at(finalized_head_hash)
		.fetch(&account_state_storage_addr)
		.await?
		.expect("Existantial deposit is set")
		.into_encoded();
	let account_state: AccountInfo = Decode::decode(&mut &account_state_encoded[..])?;

	if account_state.nonce != 0 {
		panic!("Account has non-zero nonce");
	}

	if (account_state.data.free as f64) < ext_deposit as f64 * 1.1
	/* 10% for fees */
	{
		// 10% for fees
		panic!("Account has insufficient funds");
	}
	Ok(())
}
