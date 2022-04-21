# Standard Transactions Per Second

The following cases are measured for sTPS:
- Solochain
- Single Parachain
- Multiple Pachains (5, 10, 50, and 100).

The network topologies consist of:
- 5 nodes for each network (solo, para or relay). 
- All nodes are spawn via k8 on bare metal instances.
- Each node receives 20% of the transactions over its RPC endpoint.

[Zombienet](https://github.com/paritytech/zombienet) is used for automating the setup, where the files under [`tests/stps`](../tests/spts) specify:
- `*.toml`: network topologies for each setup
- `*.feature`:  DSL test specifications
- `*tx.js`: PolkadotJS-based API calls for tx execution

The JavaScript files have simple loops for sequential initiation of transactions against a single node:
- 2 Tx per node, 10 Tx total 
- 10 Tx per node, 50 Tx total
- 20 Tx per node, 100 Tx total
- 100 Tx per node, 500 Tx total
- 200 Tx per node, 1000 Tx total
- 1000 Tx per node, 5000 Tx total
- 2000 Tx per node, 10000 Tx total

The `zombienet.sh` script automates the process of bootstrapping the setup.

# Ecosystem Performance

ToDo. The point here will be to include specific extrinsics from Parachain teams (e.g.: Acala, Astar, Moonbeam & Efinity).