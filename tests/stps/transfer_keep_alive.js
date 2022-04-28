const polkadotApi = require("@polkadot/api");
const { Keyring, encodeAddress } = require('@polkadot/keyring');
const { MAX_TOTAL_TX, MNEMONICS } = require("./constants");

// How many extrinsics will be sent at once. This requires some tweaking.
// If it is too large then they will be rejected by the RPC.
// If it is too small then it will be too slow.
const SEND_CHUNK_SIZE = 512;
// How often to check the tps.
const TPS_CHECK_INTERVAL_MS = 1000;

async function connect(apiUrl, types) {
	const provider = new polkadotApi.WsProvider(apiUrl);
	const api = new polkadotApi.ApiPromise({ provider, types });
	await api.isReady;
	return api;
}

async function run(nodeName, networkInfo, args) {
	const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
	const api = await connect(wsUri, userDefinedTypes);
	const keyring = new Keyring({ type: 'sr25519' });
	const MNEMONIC = MNEMONICS[parseInt(args)];
	const sender = keyring.addFromUri(MNEMONIC);
	console.info(`Using mnemonic '${MNEMONIC}'`);

	let transfer_amount = api.consts.balances.existentialDeposit;
	if (MAX_TOTAL_TX % SEND_CHUNK_SIZE != 0) {
		throw new Error("MAX_TOTAL_TX must be a multiple of SEND_CHUNK_SIZE");
	}

	// Holds the total number of transactions sent.
	var total_tx = 0;
	// When we last checked the tps.
	var last_check = Date.now();
	// The total_tx when we last checked the tps.
	var last_total_tx = 0;

	// Generate the receivers and pre-sign the transfer extrinsics for them.
	let receivers = [...Array(MAX_TOTAL_TX).keys()].map(gen_address);
	console.log(`Generated ${receivers.length} receiver addresses`);
	var txs = await presign(api, sender, receivers, MAX_TOTAL_TX, transfer_amount);
	var receipts = [];

	let start_time = Date.now();
	console.info("Sending transactions");
	for (var i = 0; i < txs.length; i += SEND_CHUNK_SIZE) {
		const chunk = txs.slice(i, i + SEND_CHUNK_SIZE);
		// Send the whole chunk at once.
		let promises = chunk.map(tx => tx.send());
		let new_receipts = await Promise.all(promises);
		receipts = receipts.concat(new_receipts);
		total_tx += chunk.length;

		// Print the TPS.
		const ms = Date.now() - last_check;
		if (ms >= TPS_CHECK_INTERVAL_MS) {
			const tps = (total_tx - last_total_tx) / (ms / 1000.0);
			last_total_tx = total_tx;
			last_check = Date.now();
			const percent = (total_tx / MAX_TOTAL_TX * 100);
			console.info(`[${percent.toFixed(2)}%] TX sent: ${total_tx}/${MAX_TOTAL_TX}, TPS: ${tps.toFixed(2)}`);
		}
	}
	let took_ms = Date.now() - start_time;
	let average = (total_tx / (took_ms / 1000.0));
	console.info(`Sent ${total_tx} transactions in ${took_ms} ms, average: ${average.toFixed(2)}/s`);

	// Ensure that all extrinsics got included by sending a remark and waiting for inclusion.
	// This is fine since the account nonce ensures that nothing got reordered or removed.
	// The post condition check additionally checks all the account balances.
	console.info(`Waiting for finalization of the last transaction`);
	await new Promise(async (resolve, _reject) => {
		const unsub = await api.tx.system.remark("").signAndSend(sender, { nonce: MAX_TOTAL_TX }, (result) => {
			if (result.status.isFinalized) {
				// A remark cannot error, hence inclusion is enough.
				resolve();
				unsub();
			}
		});
	});
}

// Generates an address from a seed.
function gen_address(seed) {
	let raw = Uint8Array.from(seed.toString().padStart(32, '0'));
	return encodeAddress(raw);
}

// Returns a slice of presigned transactions.
async function presign(api, sender, receivers, num, amount) {
	console.info(`Pre-signing ${num} transactions`);
	// Assume that the sender has nonce 0. This ensures that we are in the genesis state for this account.
	var nonce = 0;
	return await Promise.all(receivers.map(receiver => {
		const transfer = api.tx.balances.transferKeepAlive(receiver, amount);
		return transfer.signAsync(sender, { nonce: nonce++ });
	}));
}

module.exports = { run }
