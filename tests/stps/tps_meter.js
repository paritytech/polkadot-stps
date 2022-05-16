// Read the TPS from blocks of the node connected.
// Should be run after the benchmark is done.

const polkadotApi = require("@polkadot/api");
const { transfers_of_block } = require("./shared");

async function connect(apiUrl, types) {
	const provider = new polkadotApi.WsProvider(apiUrl);
	const api = new polkadotApi.ApiPromise({ provider, types });
	await api.isReady;
	return api;
}

async function run(nodeName, networkInfo, args) {
	const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
	const api = await connect(wsUri, userDefinedTypes);
	
	console.info("Calculating TPS...");
	await calc_tps(api);
}

async function calc_tps(api) {
	var events = [];
	const first = await api.rpc.chain.getBlockHash(1);
	const last = await api.rpc.chain.getBlock();
	// Timestamp of the last block.
	var last_now = parseInt(await api.query.timestamp.now.at(first));

	// Start at block two, assuming there is nothing in block 1 or 0.
	for (var i = 2; i < last.block.header.number; i++) {
		const hash = await api.rpc.chain.getBlockHash(i);
		const now = parseInt(await api.query.timestamp.now.at(hash));
		const time_diff = now - last_now;
		last_now = now;
		const ts = await transfers_of_block(api, i);
		if (ts > 0) {
			tps = ts / (time_diff / 1000.0);
			console.log(`Block ${i}: ${ts} transfers, ${Math.round(tps)} tps`);
		} else {
			console.warn(`Block ${i}: EMPTY`);
		}
	}
	return events;
}

module.exports = { run };
