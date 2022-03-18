# Polkadot Ecosystem Performance Benchmarks

This repository is meant to aggregate performance benchmarks from the Polkadot Ecosystem.

It is based on [Substrate Devhub Parachain Template](https://github.com/substrate-developer-hub/substrate-parachain-template/).

However, instead of `polkadot-launch`, we use [`zombienet`](https://github.com/paritytech/zombienet) for its convenient features as a DSL-based test framework.

The proposed collaborative workflow:
- Each team writes a pallet with the extrinsic calls that they wish to evaluate the performance.
- The team adds their own `.js`, `.toml` and `.feature` files into the `tests` directory, according to `zombienet`'s [network](https://github.com/paritytech/zombienet/blob/main/docs/network-definition-spec.md) and [test](https://github.com/paritytech/zombienet/blob/main/docs/test-dsl-definition-spec.md) definitions, respectively.
- Finally, the `zombienet.sh` script automates the test execution.

Note: the test DSL specs document is currently outdated and some assertion styles are missing. One of the missing features is to run custom javascript assertions, allowing to interact with the network through [@polkadot/api](https://www.npmjs.com/package/@polkadot/api).

This assertion style allows users to specify a `.js` file in order to interact with the parachain runtime. It must export a `run` function that will be called by `zombienet` with the following input parameters:
- node name
- network info
- any other argument set in the assertion

`zombienet` will execute the `.js` and assert the return value.

Here is a simple example, which already populates the directory for each team:
```
const polkadotApi = require("@polkadot/api");

async function connect(apiUrl, types) {
    const provider = new polkadotApi.WsProvider(apiUrl);
    const api = new polkadotApi.ApiPromise({ provider, types });
    await api.isReady;
    return api;
}

async function run(nodeName, networkInfo, args) {
    const {wsUri, userDefinedTypes} = networkInfo.nodesByName[nodeName];
    const api = await connect(wsUri, userDefinedTypes);
    const validator = await api.query.session.validators();
    return validator.length;
}

module.exports = { run }
```

For example, here's how to execute this test:
```
$ ./zombienet.sh test tests/acala/polkadot-js-example.feature
```
