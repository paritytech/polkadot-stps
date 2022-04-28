const polkadotApi = require("@polkadot/api");
const { MAX_TOTAL_TX } = require("./constants");

async function connect(apiUrl, types) {
	const provider = new polkadotApi.WsProvider(apiUrl);
	const api = new polkadotApi.ApiPromise({ provider, types });
	await api.isReady;
	return api;
}

// Checks post conditions.
// - Check that MAX_TOTAL_TX `Transfer` events got emitted.
async function run(nodeName, networkInfo, args) {
	const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
	const api = await connect(wsUri, userDefinedTypes);
	
	const events = await all_transfer_events(api);
	console.info(`Found ${events} Transfer events`);
	if (events != MAX_TOTAL_TX) {
		console.error(`Only found ${events} Transfer events instead of ${MAX_TOTAL_TX}`);
		await find_error(api);
		process.exit(1);
	}
}

async function all_transfer_events(api) {
	const last = await api.rpc.chain.getBlock();
	var events = 0;

	for (var i = 0; i < last.block.header.number; i++)
		events += await transfers_of_block(api, i);
	return events;
}

// Returns the number of `Balances::Transfer` events in the block.
async function transfers_of_block(api, blockNumber) {
	const blockHash = await api.rpc.chain.getBlockHash(blockNumber);
	const allRecords = await api.query.system.events.at(blockHash);

	return allRecords
		.filter(({ phase }) =>
			phase.isApplyExtrinsic
		)
		.filter(({ event }) =>
			api.events.balances.Transfer.is(event)
		).length;
}

async function find_error(api) {
	console.info("Searching for failed extrinsic...");
	const last = await api.rpc.chain.getBlock();

	for (let i = 0; i < last.block.header.number; i++) {
		await check_for_errors(api, i);
	}
	throw new Error("Failed to find the error");
}

// Checks all extrinsics in the block for success.
//
// Is rather slow and should only be used if it already determined
// that there is an error in the block.
async function check_for_errors(api, blockNumber) {
	const hash = await api.rpc.chain.getBlockHash(blockNumber);
	const signedBlock = await api.rpc.chain.getBlock();
	const allRecords = await api.query.system.events.at(signedBlock.block.header.hash);

	// map between the extrinsics and events
	signedBlock.block.extrinsics.forEach(({ method: { method, section } }, index) => {
		allRecords
			// filter the specific events based on the phase and then the
			// index of our extrinsic in the block
			.filter(({ phase }) =>
				phase.isApplyExtrinsic &&
				phase.asApplyExtrinsic.eq(index)
			)
			// test the events against the specific types we are looking for
			.forEach(({ event }) => {
				if (api.events.system.ExtrinsicFailed.is(event)) {
					// extract the data for this event
					const [dispatchError, dispatchInfo] = event.data;
					let errorInfo;

					// decode the error
					if (dispatchError.isModule) {
						// for module errors, we have the section indexed, lookup
						// (For specific known errors, we can also do a check against the
						// api.errors.<module>.<ErrorName>.is(dispatchError.asModule) guard)
						const decoded = api.registry.findMetaError(dispatchError.asModule);

						errorInfo = `${decoded.section}.${decoded.name}`;
					} else {
						// Other, CannotLookup, BadOrigin, no extra info
						errorInfo = dispatchError.toString();
					}

					throw new Error(`${section}.${method}:: ExtrinsicFailed:: ${errorInfo}`);
				}
			});
	});
}

module.exports = { run }
