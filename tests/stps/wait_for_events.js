const polkadotApi = require("@polkadot/api");
const { transfers_of_block } = require("./shared");

async function connect(apiUrl, types) {
	const provider = new polkadotApi.WsProvider(apiUrl);
	const api = new polkadotApi.ApiPromise({ provider, types });
	await api.isReady;
	return api;
}

// Checks post conditions.
// - Check that NUM_EXT `Transfer` events got emitted.
async function run(nodeName, networkInfo, args) {
	const NUM_EXT = parseInt(args);
	const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
	const api = await connect(wsUri, userDefinedTypes);
	
	var events = null;
	await new Promise(async (resolve, _reject) => {
		// Subscribe to finalized heads
		// TODO why does this not return an `unsub` function?
		const unsub = await api.rpc.chain.subscribeFinalizedHeads(async (header) => {
			const num = header.number.toNumber();

			if (events === null) {
				events = 0;
				for (var i = 0; i <= num; i++) {
					const found = await transfers_of_block(api, i);
					events += found;
					console.debug(`Block ${i} has ${found} Transfer events, need ${NUM_EXT-events} more`);
				}
			} else {
				const found = await transfers_of_block(api, num);
				events += found;
				console.debug(`Block ${num} has ${found} Transfer events, need ${NUM_EXT-events} more`);
			}

			if (events >= NUM_EXT) {
				if (events > NUM_EXT)
					console.warn(`Found too many Transfer events, ${events} > ${NUM_EXT}`);
				unsub();
				resolve();
			}
		});
	});
	
	console.info(`Found ${events} Transfer events`);
	if (events != NUM_EXT) {
		console.error(`Only found ${events} Transfer events instead of ${NUM_EXT}`);
		await find_error(api);
		process.exit(1);
	}
}

async function all_transfer_events(api) {
	const last = await api.rpc.chain.getFinalizedHead();
	var events = 0;

	for (var i = 0; i < last.block.header.number; i++) {
		const in_block = await transfers_of_block(api, i);
		events += in_block;
		console.debug(`Found ${in_block} Transfer events in block ${i}`);
	}
	return events;
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
