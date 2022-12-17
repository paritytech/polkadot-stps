use log::*;

use crate::shared::{connect, runtime, Error};

pub async fn calc_tps(node: &str, n: usize) -> Result<(), Error> {
	let api = connect(node).await?;

	let storage_timestamp_storage_addr = runtime::storage().timestamp().now();

	let block_1_hash = api.rpc().block_hash(Some(1u32.into())).await?;

	let mut last_block_timestamp = api
		.storage()
		.fetch(&storage_timestamp_storage_addr, block_1_hash)
		.await?
		.unwrap();

	let mut block_n: u32 = 2;
	let mut total_count = 0;
	let mut tps_vec = Vec::new();

	loop {
		let block_hash = api.rpc().block_hash(Some(block_n.into())).await?;

		let block_timestamp =
			api.storage().fetch(&storage_timestamp_storage_addr, block_hash).await?.unwrap();
		let time_diff = block_timestamp - last_block_timestamp;
		last_block_timestamp = block_timestamp;

		let mut tps_count = 0;
		let events = api.events().at(block_hash).await?;
		for event in events.iter().flatten() {
			if event.pallet_name() == "Balances" && event.variant_name() == "Transfer" {
				total_count += 1;
				tps_count += 1;
			}
		}

		if tps_count > 0 {
			let tps = tps_count as f32 / (time_diff as f32 / 1000.0);
			tps_vec.push(tps);
			info!("TPS on block {}: {}", block_n, tps);
		}

		block_n += 1;
		if total_count >= n {
			let avg_tps: f32 = tps_vec.iter().sum::<f32>() / tps_vec.len() as f32;
			info!("average TPS: {}", avg_tps);
			break;
		}
	}

	Ok(())
}
