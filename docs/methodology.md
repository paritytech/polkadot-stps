# Standard Transactions Per Second

The following cases are measured for sTPS:
- Solochain
- Single Parachain
- Multiple Parachains (5, 10, 50, and 100).

The network topologies consist of:
- 5 nodes for each network (solo, para and relay). 
- All nodes are spawn via k8 on bare metal instances. (ToDo: write machine specs)
- Each node receives 20% of the transactions over its RPC endpoint.

[Zombienet](https://github.com/paritytech/zombienet) is used for automating the setup, where the files under [`tests/stps`](https://github.com/paritytech/ecosystem-performance-benchmarks/tree/main/tests/stps) specify:
- `*.toml`: network topologies for each setup
- `*.feature`:  DSL test specifications
- `*.js`: PolkadotJS-based RPC calls for tx execution

The JavaScript files have simple loops for initiation of transactions towards one specific node. The Zombienet DSL on `.feature` files is responsible for specifying the different 5 nodes as targets for each `.js`.

The execution times are measured within each `.js` script and returned as a result of the `run` function.

The `zombienet.sh` script automates the process of bootstrapping the setup, namely:
- fetching the `polkadot` executable binary on a specific version.
- fetching the `zombienet-linux` executable binary on a specific version.
- installing `polkadot-js` via `npm`.
- building the parachain collator.
- ToDo: feeding the correct kubernetes parameters to `zombienet-linux`.

# Ecosystem Performance

ToDo. The point here will be to include specific extrinsics from Parachain teams (e.g.: Acala, Astar, Moonbeam & Efinity).
