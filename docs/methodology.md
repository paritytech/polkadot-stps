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
- `*tx.js`: PolkadotJS-based RPC calls for tx execution

The JavaScript files have simple loops for initiation of transactions against a single node:
- 2 Tx per node, 10 Tx per solo/para chain 
- 10 Tx per node, 50 Tx per solo/para chain
- 20 Tx per node, 100 Tx per solo/para chain
- 100 Tx per node, 500 Tx per solo/para chain
- 200 Tx per node, 1000 Tx per solo/para chain
- 1000 Tx per node, 5000 Tx per solo/para chain
- 2000 Tx per node, 10000 Tx per solo/para chain

The target execution times are manually adjusted on the `*.feature` files such that they have the smallest possible size while still returning successfully.

The `zombienet.sh` script automates the process of bootstrapping the setup.

# Ecosystem Performance

ToDo. The point here will be to include specific extrinsics from Parachain teams (e.g.: Acala, Astar, Moonbeam & Efinity).