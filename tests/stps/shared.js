// Returns the number of `Balances::Transfer` events in the block.
async function transfers_of_block(api, blockNumber) {
	const blockHash = await api.rpc.chain.getBlockHash(blockNumber);
	const allRecords = await api.query.system.events.at(blockHash);

	return allRecords
		.filter(({ phase }) =>
			phase.isApplyExtrinsic
		)
		.filter(({ event }) => {
			if (api.events.system.ExtrinsicFailed.is(event))
				throw new Error(`Extrinsic failed: ${api.events.system.ExtrinsicFailed.from(event).error.toString()}`);
			return api.events.balances.Transfer.is(event);
		}).length;
}

module.exports = { transfers_of_block };
