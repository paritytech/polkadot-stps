# Polkadot Ecosystem Performance Benchmarks

This repository is meant to aggregate performance benchmarks from the Polkadot Ecosystem.

It is based on [Substrate Devhub Parachain Template](https://github.com/substrate-developer-hub/substrate-parachain-template/).

However, instead of `polkadot-launch`, we use [`zombienet`](https://github.com/paritytech/zombienet) for its convenient features as a DSL-based test framework.

The proposed collaborative workflow:
- Each team writes a pallet with the extrinsic calls that they wish to evaluate the performance.
- The team adds their own `.toml` and `.feature` files into the `tests` directory, according to `zombienet`'s [network](https://github.com/paritytech/zombienet/blob/main/docs/network-definition-spec.md) and [test](https://github.com/paritytech/zombienet/blob/main/docs/test-dsl-definition-spec.md) definitions, respectively.
- Finally, the `zombienet.sh` script automates the test execution.
