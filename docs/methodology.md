# Standard Transactions Per Second

## Network Topologies
sTPS can be used to test:

- Substrate Solochains using the `Balances` pallet
- Relay Chain + Single Parachain, where TPS is measured for the Parachain,
- Relay Chain + Multiple Parachains (5, 10, 50, and 100).

Initially, sTPS was designed to be used with [zombienet](https://github.com/paritytech/zombienet).
With time, it became apparent that the need to measure TPS in more long-living networks was desireable. 
Therefore, sTPS also works in scenarios where not only genesis blocks are scraped.

## Zombienet
[Zombienet](https://github.com/paritytech/zombienet) is used for automating the setup, where the files under [`tests`](https://github.com/paritytech/polkadot-stps/tree/main/tests) specify:
- `*.toml`/`*.json`: network topologies for each setup
- `*.zndsl`: DSL test specifications
- `utils.js`: JS-based wrapper for Rust crate responsible for tx execution

## Pre-funded Accounts
The file `tests/funded-accounts.json` contains pre-funded accounts with enough funds, in order to satisfy the definition of sTPS. It is used as a Genesis Configuration by Zombienet. When more long-living networks are used, it is necessary to make sure this `.json` file is added to the chain-spec used for the network(s) accordingly.

## Rust Utils
The Rust crate under [`utils`](https://github.com/paritytech/polkadot-stps/tree/main/utils) has a few modules:
- `pre`: Checks the pre-conditions for sTPS measurements. More specifically, it checks the nonce and free balance of the first and last accounts in the pre-funded account list. It doesn't check the entire list in order to save time. 
- `funder`: Generates a JSON file (`tests/funded-accounts.json`) with a specific number (`n`) of pre-funded accounts.
- `sender`: Generates one pre-signed transaction per pre-funded account, and submits them in batches (to avoid clogging up the transaction pool).
- `tps`: After the every pre-funded account has submitted its transaction, this module sweeps blocks while counting how many balance transfer events were emitted in each block, and also calculating the overall average (s)TPS (by checking block timestamps). There are various arguments that can be passed to the `tps` binary, which end up defining whether it should scrape from genesis, or whether it should calculate TPS on a parachain, or relaychain basis.

### Details on scraping parablocks with `tps`

If the `--para-finality` argument is set to `true` when starting `tps`, (s)TPS is calculated for Parablocks rather than on the relaychain side. This is done by spawning two concurrent RPC clients; one for the relaychain node, and one for the collator/parachain node. By monitoring `CandidateIncluded` events on the relay-chain side, it is possible to get the hash of the most recent included Parablock on the relaychain. By sending this hash via an async mpsc channel to the parachain RPC client, it is possible to then use the collator RPC client to scrape `Transfer` events from this client. Hence, passing `--para-finality=true` sets `tps` to a concurrent system leveraging messaging passing, allowing both parachain and relaychain RPC clients to cojointly calculate the average (s)TPS for parachain blocks. Note that this assumes that the `Balances` pallet is available both in the relay- and parachain accordingly since `subxt` is used by the RPC clients. As `subxt` requires the runtime metadata at compile-time, the below section on conditional compilation will provide further details.

It is also worthwhile mentioning that currently, you cannot set `--para-finality` and `--genesis` at the same time when starting `tps`. There is an open issue to make sure this is possible: https://github.com/paritytech/polkadot-stps/issues/51, and it will be fixed soon.

### Conditional compilation
Note that the `utils/src/lib.rs` file contains a `subxt` macro which is responsible for generating the runtime metadata for the different Rust binaries defined in `utils`. More recently, the `tick-meta.scale` metadata has been included as an alternative to the `rococo-meta.scale` runtime metadata to also support connecting the `sTPS` binaries to a `polkadot-parachain` (this is usually referred to as the `tick-collator`, hence the filename) collator. This means that if you are compiling the binaries locally, you now have to specify whether to compile for `rococo`, or the `tick` collator by passing feature flags in this way:
```
$ cargo build --features tick --release
```
Note that the `rococo` metadata is used by default.

## Shell Script
The `polkadot-stps.sh` script automates the process of bootstrapping the setup, namely:
- fetching the `zombienet-linux` executable binary on a specific version.
- installing `polkadot-js` via `npm` (if not yet available).
- installing the `gcloud` toolkit (if not yet available).
- feeding the correct kubernetes parameters to `zombienet-linux`.

Being a container-based technology, Kubernetes introduces a performance overhead that should be taken into account into the interpretation of the results.
