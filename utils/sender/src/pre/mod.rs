use log::*;
use sp_core::{sr25519::Pair as SrPair, Pair};
use sp_runtime::AccountId32;
use subxt::{tx::PairSigner, PolkadotConfig};

use utils::{Api, runtime, Error, DERIVATION};

/// Check pre-conditions of accounts attributed to this sender
pub async fn pre_conditions(api: &Api, i: usize, n: usize) -> Result<(), Error> {
	info!("Sender {}: checking pre-conditions of accounts {}{} through {}{}", i, DERIVATION, i*n, DERIVATION, (i+1)*n - 1);

	for j in i*n..(i+1)*n {
		let pair: SrPair = Pair::from_string(format!("{}{}", DERIVATION, j).as_str(), None).unwrap();
		let signer: PairSigner<PolkadotConfig, SrPair> = PairSigner::new(pair);
		let account = signer.account_id();
	
		check_account(&api, account).await?;
	}

	Ok(())
}

/// Check account nonce and free balance
async fn check_account(api: &Api, account: &AccountId32) -> Result<(), Error> {
	let ext_deposit_addr = runtime::constants().balances().existential_deposit();
	let ext_deposit = api.constants().at(&ext_deposit_addr)?;

	let genesis = 0u32;
	let genesis_hash = api.rpc().block_hash(Some(genesis.into())).await?;

	// let account_state = runtime::storage().system().account(account, genesis_hash).await?;
	let account_state_storage_addr = runtime::storage().system().account(account);
	let account_state =
		api.storage().fetch(&account_state_storage_addr, genesis_hash).await?.unwrap();

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
