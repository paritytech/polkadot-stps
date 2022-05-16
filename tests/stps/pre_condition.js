const polkadotApi = require("@polkadot/api");
const { Keyring } = require('@polkadot/keyring');

async function connect(apiUrl, types) {
	const provider = new polkadotApi.WsProvider(apiUrl);
	const api = new polkadotApi.ApiPromise({ provider, types });
	await api.isReady;
	return api;
}

// Checks pre conditions.
// - Check that first and last genesis accounts have the expected balance and nonce.
async function run(nodeName, networkInfo, args) {
	const NUM_ACCOUNTS = parseInt(args);
	const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
	const api = await connect(wsUri, userDefinedTypes);

	check_account(api, "//Sender/0");
	check_account(api, "//Sender/" + (NUM_ACCOUNTS-1).toString());
}

async function check_account(api, menmonic) {
	let existential = api.consts.balances.existentialDeposit;
	const keyring = new Keyring({ type: 'sr25519' });
	const acc = keyring.addFromUri(menmonic);
	await check_address(api, acc.address, existential);
}

// Checks that the address has the correct balance and nonce.
async function check_address(api, addr, existential) {
	let { data: { free }, nonce } = await api.query.system.account(addr);
	if (nonce != 0) {
		throw new Error(`Address has a non-zero nonce: ${nonce}`);
	}
	if (free < existential * 1.1 /* 10% for fees */)
		throw new Error(`Address has insufficient funds: ${free}`);
}

module.exports = { run }
