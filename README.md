# Polkadot Ecosystem Performance Benchmarks

This repository is meant to aggregate performance benchmarks from the Polkadot Ecosystem.
It is based on [Substrate Devhub Parachain Template](https://github.com/substrate-developer-hub/substrate-parachain-template/)
Instead of `polkadot-launch`, we use [`zombienet`](https://github.com/paritytech/zombienet) for its convenient features as a DSL-based test framework.

Each team adds their own pallet with the extrinsic calls that they wish to evaluate.
The team also adds their own `.toml` and `.feature` files, according to the [network](https://github.com/paritytech/zombienet/blob/main/docs/network-definition-spec.md) and [test](https://github.com/paritytech/zombienet/blob/main/docs/test-dsl-definition-spec.md) definitions, respectively.

Finally, the `ecosystem_benchmarks.sh` script automates the test execution.