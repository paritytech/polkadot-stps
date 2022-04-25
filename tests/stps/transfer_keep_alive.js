const polkadotApi = require("@polkadot/api");
const { Keyring } = require('@polkadot/keyring');

const BOB = '5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty';

var tx_count;
var tps;

function tps_callback() {
    tps = tx_count;
    console.log(tps);

    tx_count = 0;
}

async function connect(apiUrl, types) {
    const provider = new polkadotApi.WsProvider(apiUrl);
    const api = new polkadotApi.ApiPromise({ provider, types });
    await api.isReady;
    return api;
}

async function run(nodeName, networkInfo, args) {
    const {wsUri, userDefinedTypes} = networkInfo.nodesByName[nodeName];
    const api = await connect(wsUri, userDefinedTypes);

    tx_count = 0;
    tps = 0;

    const keyring = new Keyring({ type: 'sr25519' });
    const alice = keyring.addFromUri('//Alice');

    setInterval(tps_callback, 1000); // invoke tps callback every 1s

    while (true) {
        const transfer = api.tx.balances.transfer_keep_alive(BOB, 12345);
        const hash = await transfer.signAndSend(alice);
        tx_count++;
        console.log('Transfer sent with hash', hash.toHex());
    }
}

module.exports = { run }