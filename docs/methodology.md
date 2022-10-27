# Standard Transactions Per Second

## Network Topologies
Currently, we are only measuring sTPS on a Polkadot Relay Chain with no parachains.

In the future, we want to also cover the following cases:
- Substrate Solochain
- Relay Chain + Single Parachain
- Relay Chain + Multiple Parachains (5, 10, 50, and 100).

The network topologies consist of:
- 5 nodes for each network (solo, para and relay).
- All nodes are spawn via k8 on bare metal instances. (ToDo: write machine specs)
- Each node receives 20% of the transactions over its RPC endpoint.

## Zombienet
[Zombienet](https://github.com/paritytech/zombienet) is used for automating the setup, where the files under [`tests`](https://github.com/paritytech/polkadot-stps/tree/main/tests) specify:
- `*.toml`: network topologies for each setup
- `*.zndsl`: DSL test specifications
- `utils.js`: JS-based wrapper for Rust crate responsible for tx execution

## Pre-funded Accounts
The file `tests/funded-accounts.json` contains pre-funded accounts with enough funds, in order to satisfy the definition of sTPS. It is used as a Genesis Configuration by Zombienet. 

## Rust Utils
The Rust crate under [`utils`](https://github.com/paritytech/polkadot-stps/tree/main/utils) has a few modules:
- `pre`: Checks the pre-conditions for sTPS measurements. More specifically, it checks the nonce and free balance of the first and last accounts in the pre-funded account list. It doesn't check the entire list in order to save time. 
- `funder`: Generates a JSON file (`tests/funded-accounts.json`) with a specific number (`n`) of pre-funded accounts.
- `sender`: Generates one pre-signed transaction per pre-funded account, and submits them in batches (to avoid clogging up the transaction pool).
- `tps`: After the every pre-funded account has submitted its transaction, this module sweeps every block since genesis while counting how many balance transfer events were emitted in each block, and also calculating the overall average (s)TPS (by checking block timestamps).

## Shell Script
The `polkadot-stps.sh` script automates the process of bootstrapping the setup, namely:
- fetching the `zombienet-linux` executable binary on a specific version.
- installing `polkadot-js` via `npm` (if not yet available).
- installing the `gcloud` toolkit (if not yet available).
- feeding the correct kubernetes parameters to `zombienet-linux`.

Being a container-based technology, Kubernetes introduces a performance overhead that should be taken into account into the interpretation of the results.
