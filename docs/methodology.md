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

### Parablocks

If the `--para-finality` argument is set to `true` when starting `tps`, (s)TPS is calculated for Parablocks rather than on the relaychain side. This is done by spawning two concurrent RPC clients; one for the relaychain node, and one for the collator/parachain node. By monitoring `CandidateIncluded` events on the relay-chain side, it is possible to get the hash of the most recent included Parablock. By sending this hash via an async mpsc channel to the parachain RPC client, it is possible to then use the collator RPC client to scrape `Transfer` events from this client. Hence, passing `--para-finality=true` sets `tps` to a concurrent system leveraging messaging passing, and both parachain and relaychain RPC clients to calculate the average (s)TPS for parachains.
Note that this assumes that the `Balances` pallet is available both in the relay- and parachain accordingly. See the below section on conditional compilation to understand why.

### Conditional compilation
Note that the `utils/src/lib.rs` file contains a `subxt` macro which is responsible for generating the runtime metadata for the different Rust binaries defined in `utils`. More recentlly, the `tick-meta.scale` metadata has been included as an alternative to the `metadata.scale` runtime metadata to also support connecting the `sTPS` binaries to a `polkadot-parachain` (this is usually referred to as the `tick-collator`, hence the metadata name) collator. This means that if you are compiling the binaries, you now have to specify whether to compile for `rococo`, or the `tick` collator by passing feature flags in this way for example:
```
$ cargo build --features tick --release
```
Note that the `rococo` metadata is used by default. Also, when considering the `--para-finality` argument, it is necessary to consider that both a parachain and relaychain RPC is created. Therefore, `--para-finality` assumes that the `Balances` pallet is available in both these runtimes accordingly.

## Shell Script
The `polkadot-stps.sh` script automates the process of bootstrapping the setup, namely:
- fetching the `zombienet-linux` executable binary on a specific version.
- installing `polkadot-js` via `npm` (if not yet available).
- installing the `gcloud` toolkit (if not yet available).
- feeding the correct kubernetes parameters to `zombienet-linux`.

Being a container-based technology, Kubernetes introduces a performance overhead that should be taken into account into the interpretation of the results.
