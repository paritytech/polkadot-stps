use log::*;
use sp_core::{sr25519::Pair as SrPair, Pair};
use subxt::{tx::PairSigner, utils::AccountId32, PolkadotConfig};

use utils::{runtime, Api, Error, DERIVATION};

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
		info!("Sender {}: checking account {}", i, account);
		check_account(&api, account).await?;
	}
	debug!("Sender {}: all pre-conditions checked and succeeded!", i);
	Ok(())
}

/// Use JoinSet to run prechecks in a multi-threaded way.
/// The pre_condition call is async because it fetches the chain state and hence is I/O bound.
pub async fn parallel_pre_conditions(
	api: &Api,
	threads: &usize,
	n_tx_sender: &usize,
) -> Result<(), Error> {
	let mut precheck_set = tokio::task::JoinSet::new();
	for i in 0..*threads {
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

/// Check account nonce and free balance
async fn check_account(api: &Api, account: &AccountId32) -> Result<(), Error> {
	let ext_deposit_addr = runtime::constants().balances().existential_deposit();
	let ext_deposit = api.constants().at(&ext_deposit_addr)?;
	let account_state_storage_addr = runtime::storage().system().account(account);
	let finalized_head_hash = api.rpc().finalized_head().await?;
	let account_state = api
		.storage()
		.at(finalized_head_hash)
		.fetch(&account_state_storage_addr)
		.await?
		.unwrap();

	if account_state.nonce != 0 {
		panic!("Account has non-zero nonce");
	}

	if (account_state.data.free as f32) < ext_deposit as f32 * 1.1
	/* 10% for fees */
	{
		// 10% for fees
		panic!("Account has insufficient funds");
	}
	Ok(())
}
