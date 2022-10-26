# Standard Transactions Per Second

Currently, we are only measuring sTPS on a Polkadot Relay Chain with no parachains.

In the future, we want to also cover the following cases:
- Substrate Solochain
- Relay Chain + Single Parachain
- Relay Chain + Multiple Parachains (5, 10, 50, and 100).

The network topologies consist of:
- 5 nodes for each network (solo, para and relay).
- All nodes are spawn via k8 on bare metal instances. (ToDo: write machine specs)
- Each node receives 20% of the transactions over its RPC endpoint.

[Zombienet](https://github.com/paritytech/zombienet) is used for automating the setup, where the files under [`tests`](https://github.com/paritytech/polkadot-stps/tree/main/tests) specify:
- `*.toml`: network topologies for each setup
- `*.feature`:  DSL test specifications
- `utils.js`: JS-based wrapper for Rust crate responsible for tx execution

The Rust crate under [`utils`](https://github.com/paritytech/polkadot-stps/tree/main/utils) has a few modules:
- `funder`: ToDo
- `pre`: ToDo
- `sender`: ToDo
- `tps`: ToDo

The Zombienet DSL on `.feature` files is responsible for specifying the different nodes as targets for each `utils.js`.

The `polkadot-stps.sh` script automates the process of bootstrapping the setup, namely:
- fetching the `zombienet-linux` executable binary on a specific version.
- installing `polkadot-js` via `npm`.
- feeding the correct kubernetes parameters to `zombienet-linux`.

Being a container-based technology, Kubernetes introduces a performance overhead that should be taken into account into the interpretation of the results.
